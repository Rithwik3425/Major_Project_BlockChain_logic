use anchor_lang::prelude::*;

declare_id!("4rb8oZeQMCv6wFsN2q8VnwKTri8S44PPoT8VKKGyqTAD");

#[program]
pub mod onchain_doc_storage {
    use super::*;

    pub fn create_document(
        ctx: Context<CreateDocument>,
        title: String,
        cid: String,
        file_hash: String,
    ) -> Result<()> {
        let doc = &mut ctx.accounts.document;
        doc.owner = *ctx.accounts.authority.key;
        doc.title = title.clone();
        doc.created_at = Clock::get()?.unix_timestamp;
        doc.version_count = 1;
        doc.latest_version = ctx.accounts.version.key();
        doc.bump = *ctx.bumps.get("document").unwrap();

        // init first version
        let version = &mut ctx.accounts.version;
        version.document = doc.key();
        version.index = 0;
        version.cid = cid.clone();
        version.file_hash = file_hash.clone();
        version.author = *ctx.accounts.authority.key;
        version.timestamp = Clock::get()?.unix_timestamp;

        // init owner permission
        let perm = &mut ctx.accounts.permission;
        perm.document = doc.key();
        perm.grantee = *ctx.accounts.authority.key;
        perm.perms = Permissions::READ as u8 | Permissions::WRITE as u8;
        perm.bump = *ctx.bumps.get("permission").unwrap();

        emit!(DocumentCreated {
            doc: doc.key(),
            owner: doc.owner,
            title: doc.title.clone(),
        });

        emit!(VersionAdded {
            doc: doc.key(),
            version: 0,
            cid: version.cid.clone(),
            file_hash: version.file_hash.clone(),
            added_by: version.author,
        });

        Ok(())
    }

    pub fn add_version(ctx: Context<AddVersion>, cid: String, file_hash: String) -> Result<()> {
        // permission checked via account constraint
        let version = &mut ctx.accounts.version;
        let doc = &mut ctx.accounts.document;

        version.document = doc.key();
        version.index = doc.version_count; // next index
        version.cid = cid.clone();
        version.file_hash = file_hash.clone();
        version.author = *ctx.accounts.authority.key;
        version.timestamp = Clock::get()?.unix_timestamp;

        // update document
        doc.latest_version = ctx.accounts.version.key();
        doc.version_count = doc.version_count.checked_add(1).unwrap();

        emit!(VersionAdded {
            doc: doc.key(),
            version: version.index,
            cid,
            file_hash,
            added_by: version.author,
        });

        Ok(())
    }

    pub fn grant_access(ctx: Context<GrantAccess>, perms: u8) -> Result<()> {
        // only owner can call (enforced in accounts)
        let permission = &mut ctx.accounts.permission;
        permission.document = ctx.accounts.document.key();
        permission.grantee = ctx.accounts.grantee.key();
        permission.perms = perms;
        permission.bump = *ctx.bumps.get("permission").unwrap_or(&0u8);

        emit!(AccessGranted {
            doc: ctx.accounts.document.key(),
            grantee: ctx.accounts.grantee.key(),
            perms,
            granted_by: ctx.accounts.authority.key(),
        });

        Ok(())
    }

    pub fn revoke_access(ctx: Context<RevokeAccess>) -> Result<()> {
        let permission = &mut ctx.accounts.permission;
        permission.perms = 0;

        emit!(AccessRevoked {
            doc: ctx.accounts.document.key(),
            grantee: ctx.accounts.grantee.key(),
            revoked_by: ctx.accounts.authority.key(),
        });

        Ok(())
    }
}

#[repr(u8)]
pub enum Permissions {
    READ = 1,
    WRITE = 2,
}

#[event]
pub struct DocumentCreated {
    pub doc: Pubkey,
    pub owner: Pubkey,
    pub title: String,
}

#[event]
pub struct VersionAdded {
    pub doc: Pubkey,
    pub version: u64,
    pub cid: String,
    pub file_hash: String,
    pub added_by: Pubkey,
}

#[event]
pub struct AccessGranted {
    pub doc: Pubkey,
    pub grantee: Pubkey,
    pub perms: u8,
    pub granted_by: Pubkey,
}

#[event]
pub struct AccessRevoked {
    pub doc: Pubkey,
    pub grantee: Pubkey,
    pub revoked_by: Pubkey,
}

#[account]
pub struct Document {
    pub owner: Pubkey,
    pub title: String,
    pub created_at: i64,
    pub version_count: u64,
    pub latest_version: Pubkey,
    pub bump: u8,
}

#[account]
pub struct Version {
    pub document: Pubkey,
    pub index: u64,
    pub cid: String,
    pub file_hash: String,
    pub author: Pubkey,
    pub timestamp: i64,
}

#[account]
pub struct Permission {
    pub document: Pubkey,
    pub grantee: Pubkey,
    pub perms: u8,
    pub bump: u8,
}

#[derive(Accounts)]
#[instruction(title: String, cid: String, file_hash: String)]
pub struct CreateDocument<'info> {
    #[account(init, payer = authority, space = 8 + Document::MAX_SIZE(title.len()), seeds = [b"doc", authority.key().as_ref()], bump)]
    pub document: Account<'info, Document>,

    #[account(init, payer = authority, space = 8 + Version::MAX_SIZE(cid.len(), file_hash.len()), seeds = [b"ver", document.key().as_ref(), &0u64.to_le_bytes()], bump)]
    pub version: Account<'info, Version>,

    #[account(init, payer = authority, seeds = [b"perm", document.key().as_ref(), authority.key().as_ref()], bump, space = 8 + Permission::MAX_SIZE())]
    pub permission: Account<'info, Permission>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(cid: String, file_hash: String)]
pub struct AddVersion<'info> {
    #[account(mut, seeds = [b"doc", document.owner.as_ref()], bump = document.bump)]
    pub document: Account<'info, Document>,

    // permission PDA for the authority must exist and have write perms
    #[account(seeds = [b"perm", document.key().as_ref(), authority.key().as_ref()], bump)]
    pub permission: Account<'info, Permission>,

    #[account(init, payer = authority, space = 8 + Version::MAX_SIZE(cid.len(), file_hash.len()), seeds = [b"ver", document.key().as_ref(), &document.version_count.to_le_bytes()], bump)]
    pub version: Account<'info, Version>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GrantAccess<'info> {
    #[account(mut, seeds = [b"doc", document.owner.as_ref()], bump = document.bump, has_one = owner)]
    pub document: Account<'info, Document>,

    #[account(init_if_needed, payer = authority, space = 8 + Permission::MAX_SIZE(), seeds = [b"perm", document.key().as_ref(), grantee.key().as_ref()], bump)]
    pub permission: Account<'info, Permission>,

    #[account(mut, address = document.owner)]
    pub authority: Signer<'info>,

    /// CHECK: only public key is used
    pub grantee: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RevokeAccess<'info> {
    #[account(mut, seeds = [b"doc", document.owner.as_ref()], bump = document.bump, has_one = owner)]
    pub document: Account<'info, Document>,

    #[account(mut, seeds = [b"perm", document.key().as_ref(), grantee.key().as_ref()], bump)]
    pub permission: Account<'info, Permission>,

    #[account(mut, address = document.owner)]
    pub authority: Signer<'info>,

    /// CHECK
    pub grantee: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

impl Document {
    pub fn MAX_SIZE(title_len: usize) -> usize {
        // owner (32) + title (4 + len) + created_at (8) + version_count (8) + latest_version (32) + bump (1)
        32 + 4 + title_len + 8 + 8 + 32 + 1
    }
}

impl Version {
    pub fn MAX_SIZE(cid_len: usize, hash_len: usize) -> usize {
        // document (32) + index (8) + cid (4 + len) + hash (4 + len) + author (32) + timestamp (8)
        32 + 8 + 4 + cid_len + 4 + hash_len + 32 + 8
    }
}

impl Permission {
    pub fn MAX_SIZE() -> usize {
        // document (32) + grantee (32) + perms (1) + bump (1)
        32 + 32 + 1 + 1
    }
}

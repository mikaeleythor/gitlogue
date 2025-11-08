use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use git2::{Commit as Git2Commit, Delta, DiffOptions, Oid, Repository};
use rand::seq::SliceRandom;
use std::path::Path;

pub struct GitRepository {
    repo: Repository,
}

#[derive(Debug, Clone)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    Unmodified,
}

impl FileStatus {
    pub fn as_str(&self) -> &str {
        match self {
            FileStatus::Added => "A",
            FileStatus::Deleted => "D",
            FileStatus::Modified => "M",
            FileStatus::Renamed => "R",
            FileStatus::Copied => "C",
            FileStatus::Unmodified => "U",
        }
    }
}

impl From<Delta> for FileStatus {
    fn from(delta: Delta) -> Self {
        match delta {
            Delta::Added => FileStatus::Added,
            Delta::Deleted => FileStatus::Deleted,
            Delta::Modified => FileStatus::Modified,
            Delta::Renamed => FileStatus::Renamed,
            Delta::Copied => FileStatus::Copied,
            Delta::Unmodified => FileStatus::Unmodified,
            _ => FileStatus::Modified,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
    pub diff: String,
}

#[derive(Debug, Clone)]
pub struct CommitMetadata {
    pub hash: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub message: String,
    pub changes: Vec<FileChange>,
}

impl GitRepository {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let repo = Repository::open(path)
            .context("Failed to open Git repository")?;
        Ok(Self { repo })
    }

    pub fn get_commit(&self, hash: &str) -> Result<CommitMetadata> {
        let obj = self.repo
            .revparse_single(hash)
            .context("Invalid commit hash or commit not found")?;

        let commit = obj
            .peel_to_commit()
            .context("Object is not a commit")?;

        Self::extract_metadata_with_changes(&self.repo, &commit)
    }

    pub fn random_commit(&self) -> Result<CommitMetadata> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;

        let non_merge_commits: Vec<Oid> = revwalk
            .filter_map(|oid| oid.ok())
            .filter(|oid| {
                self.repo
                    .find_commit(*oid)
                    .map(|c| c.parent_count() <= 1)
                    .unwrap_or(false)
            })
            .collect();

        if non_merge_commits.is_empty() {
            anyhow::bail!("No non-merge commits found in repository");
        }

        let oid = non_merge_commits
            .choose(&mut rand::thread_rng())
            .context("Failed to select random commit")?;

        let commit = self.repo.find_commit(*oid)?;
        Self::extract_metadata_with_changes(&self.repo, &commit)
    }

    fn extract_metadata_with_changes(
        repo: &Repository,
        commit: &Git2Commit,
    ) -> Result<CommitMetadata> {
        let hash = commit.id().to_string();
        let author = commit.author();
        let author_name = author.name().unwrap_or("Unknown").to_string();
        let timestamp = author.when().seconds();
        let date = DateTime::from_timestamp(timestamp, 0)
            .unwrap_or_else(|| Utc::now());
        let message = commit.message().unwrap_or("").trim().to_string();

        let changes = Self::extract_changes(repo, commit)?;

        Ok(CommitMetadata {
            hash,
            author: author_name,
            date,
            message,
            changes,
        })
    }

    fn extract_changes(repo: &Repository, commit: &Git2Commit) -> Result<Vec<FileChange>> {
        let commit_tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        diff_opts.context_lines(3);

        let diff = repo.diff_tree_to_tree(
            parent_tree.as_ref(),
            Some(&commit_tree),
            Some(&mut diff_opts),
        )?;

        let mut changes = Vec::new();

        diff.foreach(
            &mut |delta, _progress| {
                let status = FileStatus::from(delta.status());
                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .and_then(|p| p.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                changes.push(FileChange {
                    path,
                    status,
                    diff: String::new(),
                });
                true
            },
            None,
            None,
            None,
        )?;

        for (i, change) in changes.iter_mut().enumerate() {
            let patch = diff.get_delta(i).and_then(|_delta| {
                git2::Patch::from_diff(&diff, i).ok().flatten()
            });

            if let Some(mut patch) = patch {
                if let Ok(patch_str) = patch.to_buf() {
                    change.diff = String::from_utf8_lossy(patch_str.as_ref()).to_string();
                }
            }
        }

        Ok(changes)
    }
}

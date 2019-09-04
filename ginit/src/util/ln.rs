use into_result::{command::CommandError, IntoResult as _};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkType {
    Hard,
    Symbolic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Clobber {
    Never,
    FileOnly,
    FileOrDirectory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetStyle {
    File,
    Directory,
}

#[derive(Debug)]
pub enum ErrorCause {
    MissingFileName,
    CommandFailed(CommandError),
}

#[derive(Debug)]
pub struct Error {
    link_type: LinkType,
    force: Clobber,
    source: PathBuf,
    target: PathBuf,
    target_style: TargetStyle,
    cause: ErrorCause,
}

#[derive(Clone, Debug)]
pub struct Call<'a> {
    link_type: LinkType,
    force: Clobber,
    source: &'a Path,
    target: &'a Path,
    target_style: TargetStyle,
}

impl<'a> Call<'a> {
    pub fn new(
        link_type: LinkType,
        force: Clobber,
        source: &'a Path,
        target: &'a Path,
        target_style: TargetStyle,
    ) -> Result<Self, Error> {
        if let TargetStyle::Directory = target_style {
            // If the target is a directory, then the link name has to come from
            // the last component of the source.
            if source.file_name().is_none() {
                return Err(Error {
                    link_type,
                    force,
                    source: source.to_owned(),
                    target: target.to_owned(),
                    target_style,
                    cause: ErrorCause::MissingFileName,
                });
            }
        }
        Ok(Self {
            link_type,
            force,
            source,
            target,
            target_style,
        })
    }

    pub fn exec(self) -> Result<(), Error> {
        let mut command = Command::new("ln");
        command.arg("-h"); // don't follow symlinks
        if let LinkType::Symbolic = self.link_type {
            command.arg("-s");
        }
        match self.force {
            Clobber::FileOnly => {
                command.arg("-f");
            }
            Clobber::FileOrDirectory => {
                command.arg("-F");
            }
            _ => (),
        }
        // For the target to be interpreted as a directory, it must end in a
        // trailing slash. We can't append one using `join` or `push`, since it
        // would be interpreted as an absolute path and result in the target
        // being replaced with it: https://github.com/rust-lang/rust/issues/16507
        let target_override = if self.target_style == TargetStyle::Directory
            && (!self.target.ends_with("/") || self.target.as_os_str().is_empty())
        {
            Some(format!("{}/", self.target.display()))
        } else {
            None
        };
        command.arg(self.source);
        if let Some(target) = target_override.as_ref() {
            command.arg(target);
        } else {
            command.arg(self.target);
        }
        command.status().into_result().map_err(|err| Error {
            link_type: self.link_type,
            force: self.force,
            source: self.source.to_owned(),
            target: if let Some(target) = target_override {
                target.into()
            } else {
                self.target.to_owned()
            },
            target_style: self.target_style,
            cause: ErrorCause::CommandFailed(err),
        })
    }
}

pub fn force_symlink(
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
    target_style: TargetStyle,
) -> Result<(), Error> {
    Call::new(
        LinkType::Symbolic,
        Clobber::FileOnly,
        source.as_ref(),
        target.as_ref(),
        target_style,
    )?
    .exec()
}

pub fn force_symlink_relative(
    abs_source: impl AsRef<Path>,
    abs_target: impl AsRef<Path>,
    target_style: TargetStyle,
) -> Result<(), Error> {
    let (abs_source, abs_target) = (abs_source.as_ref(), abs_target.as_ref());
    let rel_source = super::relativize_path(abs_source, abs_target);
    if target_style == TargetStyle::Directory && rel_source.file_name().is_none() {
        if let Some(file_name) = abs_source.file_name() {
            force_symlink(rel_source, abs_target.join(file_name), TargetStyle::File)
        } else {
            Err(Error {
                link_type: LinkType::Symbolic,
                force: Clobber::FileOnly,
                source: rel_source,
                target: abs_target.to_owned(),
                target_style,
                cause: ErrorCause::MissingFileName,
            })
        }
    } else {
        force_symlink(rel_source, abs_target, target_style)
    }
}

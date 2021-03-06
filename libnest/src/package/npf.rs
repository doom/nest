use std::fs::{self, File};
use std::io::Read;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use tar::Archive;
use toml;

use super::error::{NPFExplorationError, NPFExplorationErrorKind};
use super::manifest::{Kind::Effective, Manifest};
use crate::transaction::InstructionsExecutor;

/// Structure representing a handle over a file contained in an NPF
#[derive(Debug)]
pub struct NPFFile<'explorer> {
    file: File,
    phantom: PhantomData<&'explorer NPFExplorer>,
}

impl<'explorer> NPFFile<'explorer> {
    pub(crate) fn from(file: File, phantom: PhantomData<&'explorer NPFExplorer>) -> Self {
        Self { file, phantom }
    }

    /// Retrieves the file associated with this handle, opened for reading
    pub fn file(&self) -> &File {
        &self.file
    }

    /// Retrieves the file associated with this handle, opened for reading
    pub fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }
}

/// Structure representing an NPF to allow interacting with it
#[derive(Debug)]
pub struct NPFExplorer {
    manifest: Manifest,
    path: PathBuf,
}

impl NPFExplorer {
    fn load_manifest(path: &Path) -> Result<Manifest, NPFExplorationError> {
        let mut file = File::open(path.join("manifest.toml")).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => NPFExplorationErrorKind::MissingManifest,
            _ => NPFExplorationErrorKind::FileIOError(path.to_path_buf()),
        })?;

        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|_| NPFExplorationErrorKind::FileIOError(path.to_path_buf()))?;

        Ok(toml::from_str(&content).map_err(|_| NPFExplorationErrorKind::InvalidManifest)?)
    }

    fn gen_tmp_filename<P: AsRef<Path>>(base_dir: P) -> PathBuf {
        use rand::distributions::Alphanumeric;
        use rand::{thread_rng, Rng};
        use std::iter;

        let mut rng = thread_rng();
        let name: String = iter::repeat(())
            .map(|()| rng.sample(Alphanumeric))
            .take(10)
            .collect();

        base_dir.as_ref().join(&format!("nest_{}", name))
    }

    /// Create an NPFExplorer from a path to an NPF archive and the path to the directory in which
    /// it should be extracted
    pub fn open_at<P: AsRef<Path>, Q: AsRef<Path>>(
        npf_path: P,
        extract_dir: Q,
    ) -> Result<Self, NPFExplorationError> {
        let path = Self::gen_tmp_filename(extract_dir);

        // Create a directory to extract the NPF
        fs::create_dir_all(&path).map_err(|_| NPFExplorationErrorKind::UnpackError)?;

        // Unpack the NPF
        File::open(npf_path)
            .and_then(|file| {
                let mut archive = Archive::new(&file);
                archive.unpack(&path)
            })
            .map_err(|_| NPFExplorationErrorKind::UnpackError)?;

        let manifest = Self::load_manifest(&path)?;

        Ok(Self { path, manifest })
    }

    /// Create an NPFExplorer from a path to an NPF archive
    pub fn from<P: AsRef<Path>>(npf_path: P) -> Result<Self, NPFExplorationError> {
        Self::open_at(npf_path, "/var/run/nest/")
    }

    /// Retrieves a handle over a file in the NPF
    fn open_file<P: AsRef<Path>>(&self, path: P) -> Result<NPFFile, NPFExplorationError> {
        let path = path.as_ref();
        let file = File::open(self.path.join(path)).map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => {
                NPFExplorationErrorKind::FileNotFound(path.to_path_buf())
            }
            _ => NPFExplorationErrorKind::FileIOError(path.to_path_buf()),
        })?;

        Ok(NPFFile::from(file, PhantomData))
    }

    /// Retrieves the NPF's manifest
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Retrieves a handle over the NPF's manifest.toml
    ///
    /// Note that you do not need this function to retrieve the manifest's content. It is
    /// already made available thanks to the [`NPFExplorer::manifest()`] function.
    pub fn open_manifest(&self) -> Result<NPFFile, NPFExplorationError> {
        self.open_file("manifest.toml")
    }

    /// Retrieves a handle over the NPF's data.tar.gz
    pub fn open_data(&self) -> Result<Option<NPFFile>, NPFExplorationError> {
        self.open_file("data.tar.gz").map_or_else(
            |e| match e.kind() {
                NPFExplorationErrorKind::FileNotFound(_) if self.manifest.kind() != Effective => {
                    Ok(None)
                }
                _ => Err(e),
            },
            |o| Ok(Some(o)),
        )
    }

    /// Retrieves a handle over the NPF's instructions.sh, if one exists
    pub fn open_instructions(&self) -> Result<Option<NPFFile>, NPFExplorationError> {
        self.open_file("instructions.sh").map_or_else(
            |e| match e.kind() {
                NPFExplorationErrorKind::FileNotFound(_) => Ok(None),
                _ => Err(e),
            },
            |o| Ok(Some(o)),
        )
    }

    /// Loads the NPF's instructions.sh file for execution, if one exists
    pub fn load_instructions(&self) -> Result<Option<InstructionsExecutor>, NPFExplorationError> {
        let mut file = self.open_instructions()?;

        if let Some(file) = &mut file {
            let executor =
                InstructionsExecutor::from_script_file(file.file_mut()).map_err(|_| {
                    NPFExplorationErrorKind::FileIOError(PathBuf::from("instructions.sh"))
                })?;

            Ok(Some(executor))
        } else {
            Ok(None)
        }
    }
}

impl Drop for NPFExplorer {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path).expect("unable to cleanup an extracted NPF");
    }
}

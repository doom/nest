use failure::{format_err, Error, ResultExt};
use indicatif::{ProgressBar, ProgressStyle};
use libnest::config::Config;
use libnest::lock_file::LockFileOwnership;
use libnest::transaction::InstallTransaction;

use super::download::Download;

pub fn install_package(
    config: &Config,
    trans: &mut InstallTransaction,
    ownership: &LockFileOwnership,
) -> Result<(), Error> {
    // Find the repository hosting the package
    let repo = config
        .repositories()
        .into_iter()
        .find(|repository| repository.name() == **trans.target().repository())
        .ok_or_else(|| {
            format_err!(
                "unable to find repository '{}'",
                trans.target().repository()
            )
        })?;

    // Build the target route
    let target_url = format!(
        "api/p/{}/{}/{}/download",
        trans.target().category(),
        trans.target().name(),
        trans.target().version(),
    );

    let progress_bar = ProgressBar::new(80);
    progress_bar.set_style(ProgressStyle::default_bar().template("[{pos:>3}/{len:3}] {bar:80}"));

    // Download the package archive
    progress_bar.println(format!("Downloading {}...", trans.target()));
    let download = Download::from(&target_url);
    download
        .perform_with_mirrors(
            &mut trans.create_download_file(config)?,
            &repo.config().mirrors(),
        )
        .context(format_err!(
            "unable to download package from repository '{}'",
            repo.name()
        ))?;

    // Extract and install the package
    progress_bar.println(format!("Extracting {}...", trans.target()));
    trans
        .extract(&config, ownership)
        .context(format_err!("unable to extract package"))?;

    progress_bar.finish_and_clear();
    println!("Successfully installed {}", trans.target());
    Ok(())
}
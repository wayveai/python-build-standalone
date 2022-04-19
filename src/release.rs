// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use {
    anyhow::Result,
    once_cell::sync::Lazy,
    semver::VersionReq,
    std::{
        collections::BTreeMap,
        io::{BufRead, Read, Write},
        path::{Path, PathBuf},
    },
};

/// Describes a release for a given target triple.
pub struct TripleRelease {
    /// Build suffixes to release.
    pub suffixes: Vec<&'static str>,
    /// Build suffix to use for the `install_only` artifact.
    pub install_only_suffix: &'static str,
    /// Minimum Python version this triple is released for.
    pub python_version_requirement: Option<VersionReq>,
}

pub static RELEASE_TRIPLES: Lazy<BTreeMap<&'static str, TripleRelease>> = Lazy::new(|| {
    let mut h = BTreeMap::new();

    // macOS.
    let macos_suffixes = vec!["debug", "lto", "pgo", "pgo+lto"];
    h.insert(
        "aarch64-apple-darwin",
        TripleRelease {
            suffixes: macos_suffixes.clone(),
            install_only_suffix: "pgo+lto",
            python_version_requirement: None,
        },
    );
    h.insert(
        "x86_64-apple-darwin",
        TripleRelease {
            suffixes: macos_suffixes,
            install_only_suffix: "pgo+lto",
            python_version_requirement: None,
        },
    );

    // Windows.

    // The -shared and -static parts of the triple are a lie. But the code
    // handles it fine.
    h.insert(
        "i686-pc-windows-msvc-shared",
        TripleRelease {
            suffixes: vec!["pgo"],
            install_only_suffix: "pgo",
            python_version_requirement: None,
        },
    );
    h.insert(
        "i686-pc-windows-msvc-static",
        TripleRelease {
            suffixes: vec!["noopt"],
            install_only_suffix: "noopt",
            python_version_requirement: None,
        },
    );
    h.insert(
        "x86_64-pc-windows-msvc-shared",
        TripleRelease {
            suffixes: vec!["pgo"],
            install_only_suffix: "pgo",
            python_version_requirement: None,
        },
    );
    h.insert(
        "x86_64-pc-windows-msvc-static",
        TripleRelease {
            suffixes: vec!["noopt"],
            install_only_suffix: "noopt",
            python_version_requirement: None,
        },
    );

    // Linux.
    let linux_suffixes_pgo = vec!["debug", "lto", "pgo", "pgo+lto"];
    let linux_suffixes_nopgo = vec!["debug", "lto", "noopt"];

    h.insert(
        "aarch64-unknown-linux-gnu",
        TripleRelease {
            suffixes: linux_suffixes_nopgo.clone(),
            install_only_suffix: "lto",
            python_version_requirement: Some(VersionReq::parse(">=3.9").unwrap()),
        },
    );

    h.insert(
        "i686-unknown-linux-gnu",
        TripleRelease {
            suffixes: linux_suffixes_pgo.clone(),
            install_only_suffix: "pgo+lto",
            python_version_requirement: None,
        },
    );

    h.insert(
        "x86_64-unknown-linux-gnu",
        TripleRelease {
            suffixes: linux_suffixes_pgo.clone(),
            install_only_suffix: "pgo+lto",
            python_version_requirement: None,
        },
    );
    h.insert(
        "x86_64_v2-unknown-linux-gnu",
        TripleRelease {
            suffixes: linux_suffixes_pgo.clone(),
            install_only_suffix: "pgo+lto",
            python_version_requirement: Some(VersionReq::parse(">=3.9").unwrap()),
        },
    );
    h.insert(
        "x86_64_v3-unknown-linux-gnu",
        TripleRelease {
            suffixes: linux_suffixes_pgo.clone(),
            install_only_suffix: "pgo+lto",
            python_version_requirement: Some(VersionReq::parse(">=3.9").unwrap()),
        },
    );
    h.insert(
        "x86_64_v4-unknown-linux-gnu",
        TripleRelease {
            suffixes: linux_suffixes_nopgo.clone(),
            install_only_suffix: "lto",
            python_version_requirement: Some(VersionReq::parse(">=3.9").unwrap()),
        },
    );
    h.insert(
        "x86_64-unknown-linux-musl",
        TripleRelease {
            suffixes: linux_suffixes_nopgo.clone(),
            install_only_suffix: "lto",
            python_version_requirement: None,
        },
    );
    h.insert(
        "x86_64_v2-unknown-linux-musl",
        TripleRelease {
            suffixes: linux_suffixes_nopgo.clone(),
            install_only_suffix: "lto",
            python_version_requirement: Some(VersionReq::parse(">=3.9").unwrap()),
        },
    );
    h.insert(
        "x86_64_v3-unknown-linux-musl",
        TripleRelease {
            suffixes: linux_suffixes_nopgo.clone(),
            install_only_suffix: "lto",
            python_version_requirement: Some(VersionReq::parse(">=3.9").unwrap()),
        },
    );
    h.insert(
        "x86_64_v4-unknown-linux-musl",
        TripleRelease {
            suffixes: linux_suffixes_nopgo.clone(),
            install_only_suffix: "lto",
            python_version_requirement: Some(VersionReq::parse(">=3.9").unwrap()),
        },
    );

    h
});

/// Convert a .tar.zst archive to an install only .tar.gz archive.
pub fn convert_to_install_only<W: Write>(reader: impl BufRead, writer: W) -> Result<W> {
    let dctx = zstd::stream::Decoder::new(reader)?;

    let mut tar_in = tar::Archive::new(dctx);

    let writer = flate2::write::GzEncoder::new(writer, flate2::Compression::default());

    let mut builder = tar::Builder::new(writer);

    for entry in tar_in.entries()? {
        let mut entry = entry?;

        if !entry.path_bytes().starts_with(b"python/install/") {
            continue;
        }

        let mut data = vec![];
        entry.read_to_end(&mut data)?;

        let path = entry.path()?;
        let new_path = PathBuf::from("python").join(path.strip_prefix("python/install/")?);

        let mut header = entry.header().clone();
        header.set_path(&new_path)?;
        header.set_cksum();

        builder.append(&header, std::io::Cursor::new(data))?;
    }

    Ok(builder.into_inner()?.finish()?)
}

pub fn produce_install_only(tar_zst_path: &Path) -> Result<PathBuf> {
    let buf = std::fs::read(tar_zst_path)?;

    let gz_data = convert_to_install_only(std::io::Cursor::new(buf), std::io::Cursor::new(vec![]))?
        .into_inner();

    let filename = tar_zst_path
        .file_name()
        .expect("should have filename")
        .to_string_lossy();

    let mut name_parts = filename
        .split('-')
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    let parts_len = name_parts.len();

    name_parts[parts_len - 2] = "install_only".to_string();

    let install_only_name = name_parts.join("-");
    let install_only_name = install_only_name.replace(".tar.zst", ".tar.gz");

    let dest_path = tar_zst_path.with_file_name(install_only_name);
    std::fs::write(&dest_path, &gz_data)?;

    Ok(dest_path)
}

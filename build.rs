// Copyright 2023-2025, shadow3, shadow3aaa
//
// This file is part of fas-rs.
//
// fas-rs is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.
//
// fas-rs is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with fas-rs. If not, see <https://www.gnu.org/licenses/>.

use std::{fs, io::Write};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Package {
    pub authors: Vec<String>,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Deserialize, Default)]
struct Metadata {
    #[serde(default)]
    pub fas_rs_author: String,
    #[serde(default)]
    pub fas_rs_mod_author: String,
    #[serde(default)]
    pub scene_author: String,
    #[serde(rename = "versionCodeName", default)]
    pub version_code_name: String,
    #[serde(default)]
    pub mod_version: String,
    #[serde(rename = "mod_versionCodeName", default)]
    pub mod_version_code_name: String,
}

#[derive(Deserialize)]
struct CargoConfig {
    pub package: Package,
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct UpdateJson {
    versionCode: usize,
    version: String,
    zipUrl: String,
    changelog: String,
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=update");

    let toml = fs::read_to_string("Cargo.toml")?;
    let data: CargoConfig = toml::from_str(&toml)?;

    gen_module_prop(&data)?;
    update_json(&data)?;

    Ok(())
}

fn cal_version_code(version: &str) -> Result<usize> {
    let manjor = version
        .split('.')
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid version format"))?;
    let manjor: usize = manjor.parse()?;
    let minor = version
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("Invalid version format"))?;
    let minor: usize = minor.parse()?;
    let patch = version
        .split('.')
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("Invalid version format"))?;
    let patch: usize = patch.parse()?;

    // 版本号计算规则：主版本 * 100000 + 次版本 * 1000 + 修订版本
    Ok(manjor * 100000 + minor * 1000 + patch)
}

fn gen_module_prop(data: &CargoConfig) -> Result<()> {
    let package = &data.package;
    let metadata = &package.metadata;
    let id = package.name.replace('-', "_");
    let version_code = cal_version_code(&package.version)?;

    let mut author = package.authors.join(", ");
    if !metadata.fas_rs_mod_author.is_empty() {
        if !author.is_empty() {
            author += ", ";
        }
        author += &metadata.fas_rs_mod_author;
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open("module/module.prop")?;

    writeln!(file, "id={id}")?;
    writeln!(file, "name={}", package.name)?;
    writeln!(file, "version=v{}", package.version)?;
    writeln!(file, "versionCode={version_code}")?;
    if !metadata.version_code_name.is_empty() {
        writeln!(file, "versionCodeName={}", metadata.version_code_name)?;
    }
    if !metadata.mod_version.is_empty() {
        writeln!(file, "mod_version={}", metadata.mod_version)?;
    }
    if !metadata.mod_version_code_name.is_empty() {
        writeln!(file, "mod_versionCodeName={}", metadata.mod_version_code_name)?;
    }
    writeln!(file, "author={author}")?;
    if !metadata.fas_rs_author.is_empty() {
        writeln!(file, "fas_rs_author={}", metadata.fas_rs_author)?;
    }
    if !metadata.fas_rs_mod_author.is_empty() {
        writeln!(file, "fas_rs_mod_author={}", metadata.fas_rs_mod_author)?;
    }
    if !metadata.scene_author.is_empty() {
        writeln!(file, "scene_author={}", metadata.scene_author)?;
    }
    writeln!(file, "description={}", package.description)?;

    Ok(())
}

fn update_json(data: &CargoConfig) -> Result<()> {
    let version = &data.package.version;
    let version_code = cal_version_code(version)?;
    let version = format!("v{version}");

    let zip_url =
        format!("https://github.com/shadow3aaa/fas-rs/releases/download/{version}/fas-rs.zip");

    let cn = UpdateJson {
        versionCode: version_code,
        version: version.clone(),
        zipUrl: zip_url.clone(),
        changelog: "https://github.com/shadow3aaa/fas-rs/raw/master/update/zh-CN/changelog.md"
            .into(),
    };

    let en = UpdateJson {
        versionCode: version_code,
        version,
        zipUrl: zip_url,
        changelog: "https://github.com/shadow3aaa/fas-rs/raw/master/update/en-US/changelog.md"
            .into(),
    };

    let cn = serde_json::to_string_pretty(&cn)?;
    let en = serde_json::to_string_pretty(&en)?;

    fs::write("update/update.json", cn)?;
    fs::write("update/update_en.json", en)?;

    Ok(())
}

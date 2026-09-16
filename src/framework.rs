//! Static framework conventions; never execute a project's configuration.
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Package {
    directory: PathBuf,
    next: bool,
    expo: bool,
    router: bool,
    main: Option<String>,
}
pub struct Frameworks {
    packages: Vec<Package>,
}
impl Frameworks {
    pub fn discover(files: &[PathBuf]) -> Result<Self> {
        let mut packages = Vec::new();
        for file in files
            .iter()
            .filter(|p| p.file_name().is_some_and(|n| n == "package.json"))
        {
            let value: Value = serde_json::from_str(&fs::read_to_string(file)?)
                .with_context(|| format!("invalid package manifest {}", file.display()))?;
            let dependency = |name: &str| {
                ["dependencies", "devDependencies", "peerDependencies"]
                    .iter()
                    .any(|field| value.get(field).and_then(|v| v.get(name)).is_some())
            };
            packages.push(Package {
                directory: file.parent().unwrap().to_path_buf(),
                next: dependency("next"),
                expo: dependency("expo"),
                router: dependency("expo-router")
                    && value["main"].as_str() == Some("expo-router/entry"),
                main: value["main"].as_str().map(str::to_owned),
            });
        }
        packages.sort_by_key(|p| std::cmp::Reverse(p.directory.components().count()));
        Ok(Self { packages })
    }
    fn owner(&self, file: &Path) -> Option<&Package> {
        self.packages
            .iter()
            .find(|p| file.starts_with(&p.directory))
    }
    pub fn entrypoint(&self, file: &Path) -> bool {
        let Some(package) = self.owner(file) else {
            return false;
        };
        let relative = file
            .strip_prefix(&package.directory)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if is_declaration(&relative) {
            return false;
        }
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let standard_extension = matches!(
            file.extension().and_then(|s| s.to_str()),
            Some("ts" | "tsx" | "js" | "jsx")
        );
        if package.next {
            if standard_extension {
                if let Some(route) = relative
                    .strip_prefix("app/")
                    .or_else(|| relative.strip_prefix("src/app/"))
                {
                    if !route.split('/').any(|p| p.starts_with('_'))
                        && matches!(
                            stem,
                            "page"
                                | "layout"
                                | "template"
                                | "loading"
                                | "error"
                                | "global-error"
                                | "not-found"
                                | "default"
                                | "route"
                                | "forbidden"
                                | "unauthorized"
                                | "sitemap"
                                | "robots"
                                | "manifest"
                                | "icon"
                                | "apple-icon"
                                | "opengraph-image"
                                | "twitter-image"
                        )
                    {
                        return true;
                    }
                }
                if let Some(route) = relative
                    .strip_prefix("pages/")
                    .or_else(|| relative.strip_prefix("src/pages/"))
                {
                    if !route.split('/').any(|p| p.starts_with('.')) {
                        return true;
                    }
                }
                if matches!(relative.rsplit_once('/'), None | Some(("src", _)))
                    && matches!(
                        stem,
                        "middleware" | "proxy" | "instrumentation" | "instrumentation-client"
                    )
                {
                    return true;
                }
            }
            if !relative.contains('/')
                && matches!(
                    relative.as_str(),
                    "next.config.js" | "next.config.mjs" | "next.config.ts"
                )
            {
                return true;
            }
        }
        if package.expo {
            if package.router
                && standard_extension
                && (relative.starts_with("app/") || relative.starts_with("src/app/"))
            {
                return true;
            }
            if let Some(main) = &package.main {
                if relative == main.trim_start_matches("./") {
                    return true;
                }
            } else if matches!(
                relative.as_str(),
                "index.ts" | "index.tsx" | "index.js" | "App.tsx" | "App.js" | "App.jsx"
            ) {
                return true;
            }
        }
        false
    }
    pub fn generated_type_import(&self, file: &Path, spec: &str) -> bool {
        let Some(package) = self.owner(file).filter(|p| p.next) else {
            return false;
        };
        file == package.directory.join("next-env.d.ts")
            && (spec.starts_with("./.next/types/") || spec.starts_with("./.next/dev/types/"))
            && spec.ends_with(".d.ts")
            && !spec.split('/').any(|part| part == "..")
    }
}
pub fn is_declaration(path: &str) -> bool {
    [".d.ts", ".d.mts", ".d.cts"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

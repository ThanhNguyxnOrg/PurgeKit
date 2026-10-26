use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DevClassification {
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub is_dev: bool,
}

struct Rule {
    category: &'static str,
    tag: &'static str,
    keywords: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        category: "IDE",
        tag: "ide",
        keywords: &[
            "visual studio code",
            "vscode",
            "visual studio",
            "jetbrains",
            "intellij",
            "pycharm",
            "webstorm",
            "goland",
            "clion",
            "rustrover",
            "android studio",
            "sublime text",
            "cursor",
            "zed editor",
            "neovim",
            "eclipse ide",
            "netbeans",
            "fleet",
            " ide",
        ],
    },
    Rule {
        category: "Runtime",
        tag: "runtime",
        keywords: &[
            "node.js",
            "nodejs",
            "python",
            "golang",
            "go programming",
            "rust",
            "jdk",
            "jre",
            "java ",
            "openjdk",
            ".net runtime",
            "dotnet runtime",
            "ruby",
            "php",
            "deno",
            "bun",
            "dart",
            "flutter",
            "erlang",
            "elixir",
            "lua",
            "perl",
        ],
    },
    Rule {
        category: "Package Manager",
        tag: "package-manager",
        keywords: &[
            "npm",
            "pnpm",
            "yarn",
            "cargo",
            "pip",
            "nuget",
            "composer",
            "chocolatey",
            "scoop",
            "winget",
            "package manager",
        ],
    },
    Rule {
        category: "Build Tool",
        tag: "build-tool",
        keywords: &[
            "cmake",
            "ninja",
            "msbuild",
            "gradle",
            "maven",
            "bazel",
            "msys2",
            "mingw",
            "make",
            "visual studio build tools",
            "visual studio installer",
            "build tools",
        ],
    },
    Rule {
        category: "Container",
        tag: "container",
        keywords: &[
            "docker",
            "podman",
            "rancher",
            "kubernetes",
            "kubectl",
            "minikube",
            "windows subsystem for linux",
            "wsl",
        ],
    },
    Rule {
        category: "Database",
        tag: "database",
        keywords: &[
            "sql server",
            "postgresql",
            "postgres",
            "mysql",
            "mongodb",
            "redis",
            "sqlite",
            "mariadb",
            "elasticsearch",
            "oracle database",
        ],
    },
    Rule {
        category: "Cloud CLI",
        tag: "cloud-cli",
        keywords: &[
            "aws cli",
            "aws command line",
            "azure cli",
            "google cloud sdk",
            "gcloud",
            "firebase cli",
            "heroku cli",
            "vercel cli",
            "netlify cli",
        ],
    },
    Rule {
        category: "Terminal",
        tag: "terminal",
        keywords: &[
            "windows terminal",
            "powershell",
            "git bash",
            "cmder",
            "alacritty",
            "wezterm",
            "conemu",
        ],
    },
    Rule {
        category: "Game Dev",
        tag: "game-dev",
        keywords: &[
            "unity",
            "unreal engine",
            "godot",
            "blender",
            "game engine",
        ],
    },
    Rule {
        category: "SDK",
        tag: "sdk",
        keywords: &[
            "windows sdk",
            "android sdk",
            "dotnet sdk",
            "java sdk",
            "software development kit",
            " sdk",
        ],
    },
    Rule {
        category: "API Client",
        tag: "api-client",
        keywords: &["postman", "insomnia", "bruno", "httpie"],
    },
    Rule {
        category: "CLI",
        tag: "cli",
        keywords: &[
            "jq",
            "curl",
            "wget",
            "ripgrep",
            "fzf",
            "git",
            "github cli",
            "gh ",
            "bash",
            "ssh",
        ],
    },
    Rule {
        category: "Version Manager",
        tag: "version-manager",
        keywords: &["rustup", "nvm", "fnm", "elan"],
    },
];

fn matches_keyword(haystack: &str, keyword: &str) -> bool {
    let keyword = keyword.trim().to_lowercase();
    if keyword.is_empty() {
        return false;
    }

    if keyword.chars().all(|character| character.is_alphanumeric()) {
        haystack
            .split(|character: char| !character.is_alphanumeric())
            .any(|word| word == keyword)
    } else {
        haystack.contains(&keyword)
    }
}

fn canonical_tag(keyword: &str) -> Option<&'static str> {
    match keyword {
        "visual studio code" | "vscode" => Some("vscode"),
        "visual studio" => Some("visual-studio"),
        "jetbrains" | "intellij" | "pycharm" | "webstorm" | "goland" | "clion" | "rustrover" => {
            Some("jetbrains")
        }
        "node.js" | "nodejs" => Some("node"),
        "python" => Some("python"),
        "golang" | "go programming" => Some("go"),
        "rust" => Some("rust"),
        "jdk" | "jre" | "openjdk" => Some("java"),
        ".net runtime" | "dotnet runtime" | "dotnet sdk" => Some("dotnet"),
        "docker" => Some("docker"),
        "windows subsystem for linux" | "wsl" => Some("wsl"),
        "windows terminal" => Some("windows-terminal"),
        "powershell" => Some("powershell"),
        "unity" => Some("unity"),
        "unreal engine" => Some("unreal"),
        "cmake" => Some("cmake"),
        "git bash" => Some("git"),
        _ => None,
    }
}

pub fn classify(
    name: &str,
    publisher: Option<&str>,
    _install_location: Option<&str>,
    source: &str,
    managed_by: Option<&str>,
) -> DevClassification {
    // Install paths are noisy: a Chocolatey shim path would classify every
    // tool installed through Chocolatey as "Package Manager". Classify from
    // the app identity instead; manager-owned sources are handled explicitly
    // below.
    let publisher = if source == "cli" {
        ""
    } else {
        publisher.unwrap_or_default()
    };
    let haystack = format!("{} {}", name, publisher).to_lowercase();

    let mut categories = Vec::new();
    let mut tags = Vec::new();

    for rule in RULES {
        if rule.category == "IDE" && haystack.contains("visual studio installer") {
            continue;
        }
        if rule
            .keywords
            .iter()
            .any(|keyword| matches_keyword(&haystack, keyword))
        {
            if !categories.iter().any(|category| category == rule.category) {
                categories.push(rule.category.to_string());
            }
            if !tags.iter().any(|tag| tag == rule.tag) {
                tags.push(rule.tag.to_string());
            }

            for keyword in rule.keywords {
                if matches_keyword(&haystack, keyword) {
                    if let Some(tag) = canonical_tag(keyword) {
                        if !tags.iter().any(|existing| existing == tag) {
                            tags.push(tag.to_string());
                        }
                        break;
                    }
                }
            }
        }
    }

    if matches!(source, "cli" | "toolchain") && !categories.iter().any(|c| c == "CLI") {
        categories.push("CLI".to_string());
    }
    if matches!(managed_by, Some("rustup" | "nvm" | "fnm"))
        && !categories.iter().any(|c| c == "Version Manager")
    {
        categories.push("Version Manager".to_string());
    }

    let is_dev = !categories.is_empty() || matches!(source, "cli" | "toolchain");
    DevClassification {
        categories,
        tags,
        is_dev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_dev_tools() {
        let vscode = classify("Visual Studio Code", Some("Microsoft"), None, "registry", None);
        assert!(vscode.categories.contains(&"IDE".to_string()));
        assert!(vscode.tags.contains(&"vscode".to_string()));

        let node = classify("Node.js", Some("OpenJS Foundation"), None, "registry", None);
        assert!(node.categories.contains(&"Runtime".to_string()));
        assert!(node.tags.contains(&"node".to_string()));

        let docker = classify("Docker Desktop", Some("Docker Inc."), None, "registry", None);
        assert!(docker.categories.contains(&"Container".to_string()));

        let cli = classify("some-cli", Some("npm"), None, "cli", Some("npm"));
        assert!(cli.is_dev);
        assert!(cli.categories.contains(&"CLI".to_string()));
    }

    #[test]
    fn does_not_mark_office_as_dev() {
        let office = classify("Microsoft Office", Some("Microsoft"), None, "registry", None);
        assert!(!office.is_dev);
    }

    #[test]
    fn avoids_common_false_positives() {
        let jdk = classify(
            "Eclipse Temurin JDK with Hotspot 25",
            Some("Eclipse Adoptium"),
            None,
            "registry",
            None,
        );
        assert!(jdk.categories.contains(&"Runtime".to_string()));
        assert!(!jdk.categories.contains(&"IDE".to_string()));

        let jq = classify(
            "jq",
            Some("jqlang"),
            Some(r"C:\ProgramData\chocolatey\bin\jq.exe"),
            "registry",
            None,
        );
        assert!(jq.categories.contains(&"CLI".to_string()));
        assert!(!jq.categories.contains(&"Package Manager".to_string()));

        let installer = classify(
            "Microsoft Visual Studio Installer",
            Some("Microsoft"),
            None,
            "registry",
            None,
        );
        assert!(installer.categories.contains(&"Build Tool".to_string()));
        assert!(!installer.categories.contains(&"IDE".to_string()));
    }
}

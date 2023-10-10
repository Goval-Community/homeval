/*
message ToolchainConfigs {
    string entrypoint = 1;
    repeated RunOption runs = 2;
    repeated DebuggerOption debuggers = 3;
    repeated LanguageServerOption languageServers = 4;
    repeated PackagerOption packagers = 5;
}
*/

use std::collections::HashMap;

use goval;
pub use serde::{Deserialize, Serialize};

pub mod toolchain {
    use super::*;

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct ToolchainConfigs {
        pub entrypoint: Option<String>,
        pub runs: Vec<RunOption>,
        pub debuggers: Vec<DebuggerOption>,
        pub language_servers: Vec<LanguageServerOption>,
        pub packagers: Vec<PackagerOption>,
    }

    /*
    message RunOption {
        string id = 1;
        string name = 2;
        bool fileParam = 3;
        string language = 4;
        FileTypeAttrs fileTypeAttrs = 5;
    }
    */

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct RunOption {
        pub id: Option<String>,
        pub name: Option<String>,
        pub file_param: Option<bool>,
        pub language: Option<String>,
        pub file_type_attrs: Option<FileTypeAttrs>,
    }

    /*
    message DebuggerOption {
        string id = 1;
        string name = 2;
        bool fileParam = 3;
        string language = 4;
        FileTypeAttrs fileTypeAttrs = 5;
    }
    */

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct DebuggerOption {
        pub id: Option<String>,
        pub name: Option<String>,
        pub file_param: Option<bool>,
        pub language: Option<String>,
        pub file_type_attrs: Option<FileTypeAttrs>,
    }

    /*
    message LanguageServerOption {
        string id = 1;
        string name = 2;
        string language = 3;
        FileTypeAttrs fileTypeAttrs = 4;
        LanguageServerConfig config = 5;
    }
    */

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct LanguageServerOption {
        pub id: Option<String>,
        pub name: Option<String>,
        pub language: Option<String>,
        pub file_type_attrs: Option<FileTypeAttrs>,
        pub config: Option<LanguageServerConfig>,
    }

    /*
    message FileTypeAttrs {
        repeated string extensions = 1;
        repeated string files = 2;
        string filePattern = 3;
    }
    */

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct FileTypeAttrs {
        pub extensions: Vec<String>,
        pub files: Vec<String>,
        pub file_pattern: Option<String>,
    }

    /*
    message PackagerOption {
        string id = 1;
        string name = 2;
        string language = 3;
        repeated string packagerFiles = 4;
        bool enabledForHosting = 5;
        bool packageSearch = 6;
        bool guessImports = 7;
    }
    */

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct PackagerOption {
        pub id: Option<String>,
        pub name: Option<String>,
        pub language: Option<String>,
        pub packager_files: Vec<String>,
        pub enabled_for_hosting: Option<bool>,
        pub package_search: Option<bool>,
        pub guess_imports: Option<bool>,
    }
}

pub mod dotreplit {
    use super::*;

    /*
    Exec run = 1;
    string language = 4;
    string entrypoint = 8;
    map<string,DotReplitLanguage> languages = 9;
    repeated string hidden = 11;
    */
    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct DotReplit {
        pub run: Option<Exec>,
        pub language: Option<String>,
        pub entrypoint: Option<String>,
        pub languages: Option<HashMap<String, DotReplitLanguage>>,
        pub hidden: Option<Vec<String>>,
    }

    impl From<DotReplit> for goval::DotReplit {
        fn from(val: DotReplit) -> Self {
            let mut ret = goval::DotReplit::default();

            if let Some(run) = val.run {
                // let mut inner = goval::Exec::default();
                // inner.args = vec!["sh".into(), "-c".into(), run];
                ret.run = Some(run.into());
            }

            if let Some(lang) = val.language {
                ret.language = lang;
            }

            if let Some(entrypoint) = val.entrypoint {
                ret.entrypoint = entrypoint;
            }

            if let Some(languages) = val.languages {
                let mut inner = HashMap::new();

                for (lang, data) in languages.iter() {
                    inner.insert(lang.into(), data.clone().into());
                }

                ret.languages = inner
            }

            if let Some(hidden) = val.hidden {
                ret.hidden = hidden;
            }

            ret
        }
    }
    /*
        message DotReplitLanguage {
            string pattern = 1;
            string syntax = 2;
            LanguageServerConfig languageServer = 3;
        }
    */

    #[derive(Serialize, Deserialize, Clone, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct DotReplitLanguage {
        pub pattern: Option<String>,
        pub syntax: Option<String>,
        pub language_server: Option<LanguageServerConfig>,
    }

    impl From<DotReplitLanguage> for goval::DotReplitLanguage {
        fn from(val: DotReplitLanguage) -> Self {
            let mut ret = goval::DotReplitLanguage::default();

            if let Some(pattern) = val.pattern {
                ret.pattern = pattern;
            }

            if let Some(syntax) = val.syntax {
                ret.syntax = syntax;
            }

            if let Some(language_server) = val.language_server {
                ret.language_server = Some(language_server.into());
            }

            ret
        }
    }
}

/*
message LanguageServerConfig {
    Exec startCommand = 1;
    string configurationJson = 2;
    string initializationOptionsJson = 3;
}
*/

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LanguageServerConfig {
    #[serde(rename = "start")]
    pub start_command: Option<Exec>,
    pub configuration_json: Option<String>,
    pub initialization_options_json: Option<String>,
}

impl From<LanguageServerConfig> for goval::LanguageServerConfig {
    fn from(val: LanguageServerConfig) -> Self {
        let mut ret = goval::LanguageServerConfig::default();

        if let Some(start_command) = val.start_command {
            // let mut inner = goval::Exec::default();
            // inner.args = vec!["sh".into(), "-c".into(), start_command];
            // ret.start_command = Some(inner);
            ret.start_command = Some(start_command.into());
        }

        if let Some(configuration_json) = val.configuration_json {
            ret.configuration_json = configuration_json;
        }

        if let Some(initialization_options_json) = val.initialization_options_json {
            ret.initialization_options_json = initialization_options_json;
        };

        ret
    }
}
/*
message Exec {
    enum Lifecycle {
        NON_BLOCKING = 0;
        BLOCKING = 1;
        STDIN = 2;
    }

    repeated string args = 1;
    map<string,string> env = 2;
    bool blocking = 3;
    Exec.Lifecycle lifecycle = 6;
    bool splitStderr = 4;
    bool splitLogs = 5;
}
*/

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ExecLifecycle {
    NonBlocking,
    Stdin,
    Blocking,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Exec {
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub blocking: Option<bool>,
    pub lifecycle: Option<ExecLifecycle>,
    pub split_stderr: Option<bool>,
    pub split_logs: Option<bool>,
}

impl From<Exec> for goval::Exec {
    fn from(val: Exec) -> Self {
        let mut ret = goval::Exec::default();

        if let Some(args) = val.args {
            ret.args = args;
        }

        if let Some(env) = val.env {
            ret.env = env;
        }

        if let Some(blocking) = val.blocking {
            ret.blocking = blocking;
        }

        if let Some(split_stderr) = val.split_stderr {
            ret.split_stderr = split_stderr;
        }

        if let Some(split_logs) = val.split_logs {
            ret.split_logs = split_logs;
        }

        if let Some(lifecycle) = val.lifecycle {
            ret.lifecycle = match lifecycle {
                ExecLifecycle::NonBlocking => goval::exec::Lifecycle::NonBlocking,
                ExecLifecycle::Blocking => goval::exec::Lifecycle::Blocking,
                ExecLifecycle::Stdin => goval::exec::Lifecycle::Stdin,
            }
            .into();
        }

        ret
    }
}

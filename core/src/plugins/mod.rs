use mlua::{Lua, Table, Value, Function};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub id: String,
    pub title: String,
    pub source: String,
    pub exec_path: String,
    pub launch_args: Vec<String>,
    pub cover_art: Option<String>,
}

pub struct Plugin {
    pub id: String,
    pub name: String,
    pub permissions: Vec<String>,
    lua: Lua,
    plugin_table: mlua::RegistryKey,
}

pub struct PluginManager {
    plugins_dir: PathBuf,
    pub plugins: HashMap<String, Plugin>,
    pub granted_permissions: HashMap<String, Vec<String>>,
    cache: Arc<Mutex<HashMap<String, String>>>,
    cache_file: PathBuf,
}

impl PluginManager {
    pub fn new<P: AsRef<Path>>(plugins_dir: P, cache_file: P) -> Self {
        let mut cache = HashMap::new();
        if cache_file.as_ref().exists() {
            if let Ok(content) = std::fs::read_to_string(cache_file.as_ref()) {
                if let Ok(parsed) = serde_json::from_str(&content) {
                    cache = parsed;
                }
            }
        }

        Self {
            plugins_dir: plugins_dir.as_ref().to_path_buf(),
            plugins: HashMap::new(),
            granted_permissions: HashMap::new(),
            cache: Arc::new(Mutex::new(cache)),
            cache_file: cache_file.as_ref().to_path_buf(),
        }
    }

    pub fn save_cache(&self) {
        if let Ok(cache) = self.cache.lock() {
            if let Ok(content) = serde_json::to_string_pretty(&*cache) {
                let _ = std::fs::create_dir_all(self.cache_file.parent().unwrap());
                let _ = std::fs::write(&self.cache_file, content);
            }
        }
    }

    pub fn load_all_plugins(&mut self) -> Result<(), String> {
        self.plugins.clear();
        if !self.plugins_dir.exists() {
            let _ = std::fs::create_dir_all(&self.plugins_dir);
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.plugins_dir)
            .map_err(|e| format!("Failed to read plugins dir: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "lua") {
                if let Err(e) = self.load_plugin(&path) {
                    println!("Failed to load plugin {}: {}", path.display(), e);
                }
            }
        }
        Ok(())
    }

    pub fn load_plugin(&mut self, path: &Path) -> Result<(), String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Read file failed: {}", e))?;

        // Extract ID first by reading metadata or running in dry-run sandbox
        let dry_lua = Lua::new();
        let dry_table: Table = dry_lua.load(&content).eval()
            .map_err(|e| format!("Lua evaluation failed: {}", e))?;
        let metadata: Table = dry_table.get("metadata")
            .map_err(|_| "Missing plugin metadata table".to_string())?;
        let id: String = metadata.get("id")
            .map_err(|_| "Missing plugin metadata.id".to_string())?;
        let name: String = metadata.get("name")
            .map_err(|_| "Missing plugin metadata.name".to_string())?;
        let permissions: Vec<String> = metadata.get("permissions").unwrap_or_default();

        // Check/assign permissions
        let granted_perms = self.granted_permissions.entry(id.clone())
            .or_insert_with(|| permissions.clone()); // By default grant what's requested for testing

        // Create the real sandboxed environment
        let lua = Lua::new();
        
        let reg_key = {
            // Remove unsafe globals
            let globals = lua.globals();
            globals.set("dofile", Value::Nil).unwrap();
            globals.set("loadfile", Value::Nil).unwrap();
            globals.set("load", Value::Nil).unwrap();
            globals.set("loadstring", Value::Nil).unwrap();
            globals.set("require", Value::Nil).unwrap();
            
            if let Ok(os) = globals.get::<_, Table>("os") {
                let safe_os = lua.create_table().unwrap();
                safe_os.set("time", os.get::<_, Function>("time").unwrap()).unwrap();
                safe_os.set("date", os.get::<_, Function>("date").unwrap()).unwrap();
                safe_os.set("difftime", os.get::<_, Function>("difftime").unwrap()).unwrap();
                globals.set("os", safe_os).unwrap();
            }
            globals.set("io", Value::Nil).unwrap();
            globals.set("package", Value::Nil).unwrap();

            // Create grapevine host namespace
            let grapevine = lua.create_table().unwrap();

            // http_get
            let http_get = lua.create_function(|_, url: String| {
                let res = reqwest::blocking::get(&url)
                    .map_err(|e| mlua::Error::RuntimeError(format!("HTTP GET failed: {}", e)))?;
                let body = res.text()
                    .map_err(|e| mlua::Error::RuntimeError(format!("Read HTTP body failed: {}", e)))?;
                Ok(body)
            }).unwrap();
            grapevine.set("http_get", http_get).unwrap();

            // http_download
            let http_download = lua.create_function(|_, (url, dest): (String, String)| {
                let mut res = reqwest::blocking::get(&url)
                    .map_err(|e| mlua::Error::RuntimeError(format!("HTTP download failed: {}", e)))?;
                let mut file = std::fs::File::create(&dest)
                    .map_err(|e| mlua::Error::RuntimeError(format!("Create file failed: {}", e)))?;
                res.copy_to(&mut file)
                    .map_err(|e| mlua::Error::RuntimeError(format!("Write file failed: {}", e)))?;
                Ok(())
            }).unwrap();
            grapevine.set("http_download", http_download).unwrap();

            // read_file / write_file
            let has_read_fs = granted_perms.contains(&"read_fs".to_string());
            let read_file = lua.create_function(move |_, path: String| {
                if !has_read_fs {
                    return Err(mlua::Error::RuntimeError("Permission denied: read_fs".to_string()));
                }
                std::fs::read_to_string(&path)
                    .map_err(|e| mlua::Error::RuntimeError(format!("Read file failed: {}", e)))
            }).unwrap();
            grapevine.set("read_file", read_file).unwrap();

            let has_write_fs = granted_perms.contains(&"write_fs".to_string());
            let write_file = lua.create_function(move |_, (path, content): (String, String)| {
                if !has_write_fs {
                    return Err(mlua::Error::RuntimeError("Permission denied: write_fs".to_string()));
                }
                std::fs::write(&path, content)
                    .map_err(|e| mlua::Error::RuntimeError(format!("Write file failed: {}", e)))
            }).unwrap();
            grapevine.set("write_file", write_file).unwrap();

            // read_cache / write_cache
            let cache_ref_read = Arc::clone(&self.cache);
            let read_cache = lua.create_function(move |_, key: String| {
                if let Ok(cache) = cache_ref_read.lock() {
                    Ok(cache.get(&key).cloned())
                } else {
                    Ok(None)
                }
            }).unwrap();
            grapevine.set("read_cache", read_cache).unwrap();

            let cache_ref_write = Arc::clone(&self.cache);
            let write_cache = lua.create_function(move |_, (key, val): (String, String)| {
                if let Ok(mut cache) = cache_ref_write.lock() {
                    cache.insert(key, val);
                }
                Ok(())
            }).unwrap();
            grapevine.set("write_cache", write_cache).unwrap();

            // grapevine.execute
            let has_execute = granted_perms.contains(&"execute".to_string());
            let execute = lua.create_function(move |_, (cmd, args): (String, Vec<String>)| {
                if !has_execute {
                    return Err(mlua::Error::RuntimeError("Permission denied: execute".to_string()));
                }
                let output = std::process::Command::new(cmd)
                    .args(args)
                    .output()
                    .map_err(|e| mlua::Error::RuntimeError(format!("Process execute failed: {}", e)))?;
                
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                
                let res_table = std::collections::HashMap::from([
                    ("stdout".to_string(), stdout),
                    ("stderr".to_string(), stderr),
                    ("exit_code".to_string(), output.status.code().unwrap_or(-1).to_string()),
                ]);
                Ok(res_table)
            }).unwrap();
            grapevine.set("execute", execute).unwrap();

            globals.set("grapevine", grapevine).unwrap();

            // Load the actual script and keep a registry reference to the plugin table
            let plugin_table: Table = lua.load(&content).eval()
                .map_err(|e| format!("Real evaluation failed: {}", e))?;
            lua.create_registry_value(plugin_table)
                .map_err(|e| format!("Registry key creation failed: {}", e))?
        };

        self.plugins.insert(id.clone(), Plugin {
            id,
            name,
            permissions: permissions.clone(),
            lua,
            plugin_table: reg_key,
        });

        Ok(())
    }

    pub fn search(&self, plugin_id: &str, query: &str) -> Result<Vec<GameInfo>, String> {
        let plugin = self.plugins.get(plugin_id)
            .ok_or_else(|| format!("Plugin {} not found", plugin_id))?;
        
        let globals = plugin.lua.globals();
        let plugin_table: Table = plugin.lua.registry_value(&plugin.plugin_table)
            .map_err(|e| e.to_string())?;

        let search_fn: Function = plugin_table.get("search")
            .map_err(|_| "Plugin search function not found".to_string())?;

        let results_val: Value = search_fn.call((query,))
            .map_err(|e| format!("Search call failed: {}", e))?;

        let mut games = Vec::new();
        if let Value::Table(t) = results_val {
            let len = t.len().map_err(|e| e.to_string())?;
            for i in 1..=len {
                let entry: Table = t.get(i).map_err(|e| e.to_string())?;
                
                let id: String = entry.get("id").map_err(|e| e.to_string())?;
                let title: String = entry.get("title").map_err(|e| e.to_string())?;
                let source: String = entry.get("source").map_err(|e| e.to_string())?;
                let exec_path: String = entry.get("exec_path").map_err(|e| e.to_string())?;
                let launch_args: Vec<String> = entry.get("launch_args").unwrap_or_default();
                let cover_art: Option<String> = entry.get("cover_art").ok();

                games.push(GameInfo {
                    id,
                    title,
                    source,
                    exec_path,
                    launch_args,
                    cover_art,
                });
            }
        }
        Ok(games)
    }

    pub fn list_installed(&self, plugin_id: &str) -> Result<Vec<GameInfo>, String> {
        let plugin = self.plugins.get(plugin_id)
            .ok_or_else(|| format!("Plugin {} not found", plugin_id))?;
        
        let plugin_table: Table = plugin.lua.registry_value(&plugin.plugin_table)
            .map_err(|e| e.to_string())?;

        let list_fn: Function = plugin_table.get("list_installed")
            .map_err(|_| "Plugin list_installed function not found".to_string())?;

        let results_val: Value = list_fn.call(())
            .map_err(|e| format!("list_installed call failed: {}", e))?;

        let mut games = Vec::new();
        if let Value::Table(t) = results_val {
            let len = t.len().map_err(|e| e.to_string())?;
            for i in 1..=len {
                let entry: Table = t.get(i).map_err(|e| e.to_string())?;
                
                let id: String = entry.get("id").map_err(|e| e.to_string())?;
                let title: String = entry.get("title").map_err(|e| e.to_string())?;
                let source: String = entry.get("source").map_err(|e| e.to_string())?;
                let exec_path: String = entry.get("exec_path").map_err(|e| e.to_string())?;
                let launch_args: Vec<String> = entry.get("launch_args").unwrap_or_default();
                let cover_art: Option<String> = entry.get("cover_art").ok();

                games.push(GameInfo {
                    id,
                    title,
                    source,
                    exec_path,
                    launch_args,
                    cover_art,
                });
            }
        }
        Ok(games)
    }
}

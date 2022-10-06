use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::thread;
use std::thread::ScopedJoinHandle;

use anyhow::format_err;
use anyhow::Result;
use log::error;

use super::dependency::DependencyManager;
use crate::core::Dependency;
use crate::core::DependencyLock;
use crate::core::Repository;
use crate::core::Spec;
use crate::core::SpecLock;

type ActionFn = dyn Fn(&VendorManager, Dependency) -> Result<DependencyLock> + Sync + Send;

pub struct VendorManager {
    cache: PathBuf,
    spec: Arc<RwLock<Spec>>,
    lock: Arc<RwLock<SpecLock>>,
}

impl VendorManager {
    pub fn new<P: AsRef<Path>>(
        cache: P,
        spec: Arc<RwLock<Spec>>,
        lock: Arc<RwLock<SpecLock>>,
    ) -> Self {
        VendorManager {
            cache: cache.as_ref().to_owned(),
            spec,
            lock,
        }
    }

    pub fn install(self) -> Result<()> {
        self.execute(Arc::new(inner_install))
    }

    pub fn update(self) -> Result<()> {
        self.execute(Arc::new(inner_update))
    }

    fn execute(self, action: Arc<ActionFn>) -> Result<()> {
        recreate_vendor_path(&self.spec.read().unwrap().vendor)?;

        let deps = self.spec.read().unwrap().deps.clone();

        let woop = Arc::new(&self);

        thread::scope(|s| {
            let mut handles: Vec<ScopedJoinHandle<Result<DependencyLock>>> = vec![];

            for dependency in deps.into_iter() {
                handles.push(s.spawn(|| action(&woop, dependency)));
            }

            for handle in handles.into_iter() {
                if let Ok(result) = handle.join() {
                    self.update_lock(result)
                }
            }
        });

        Ok(())
    }

    fn update_lock(&self, result: Result<DependencyLock>) {
        match result {
            Ok(updated_dependency_lock) => {
                self.lock.write().unwrap().add(updated_dependency_lock);
            }
            Err(err) => {
                error!("failed importing: {}", err);
            }
        }
    }
}

fn inner_install(manager: &VendorManager, dependency: Dependency) -> Result<DependencyLock> {
    let repository = Repository::new(&manager.cache, &dependency);
    let binding = manager.lock.read().unwrap();
    let dependency_lock = binding.find_dep(&dependency.url);
    let binding = manager.spec.read().unwrap();
    let dependency_manager =
        DependencyManager::new(&binding, &dependency, dependency_lock, &repository);

    dependency_manager.install(&manager.spec.read().unwrap().vendor)
}

fn inner_update(manager: &VendorManager, dependency: Dependency) -> Result<DependencyLock> {
    let repository = Repository::new(&manager.cache, &dependency);
    let binding = manager.spec.read().unwrap();
    let dependency_manager = DependencyManager::new(&binding, &dependency, None, &repository);

    dependency_manager.update(&manager.spec.read().unwrap().vendor)
}

fn recreate_vendor_path<P: AsRef<Path>>(path: P) -> Result<()> {
    delete_vendor_path(&path)?;
    create_vendor_path(&path)
}

fn delete_vendor_path<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|err| format_err!("cannot reset vendor folder: {}", err))?
    }
    Ok(())
}

fn create_vendor_path<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        fs::create_dir_all(path).map_err(|err| {
            format_err!(
                "cannot create vendor folder '{name}': {err}",
                name = path.display(),
                err = err
            )
        })?
    }
    if !path.is_dir() {
        return Err(format_err!(
            "vendor path '{}' already exists, and it's not a directory",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::core::tests::test_util::tempdir;
    use crate::core::tests::test_util::write_to;

    #[test]
    fn test_ensure_vendor_empty_root() {
        let root = tempdir();
        let vendor = root.path().join("vendor");

        match create_vendor_path(&vendor) {
            Ok(()) => {
                assert!(vendor.exists());
                assert!(vendor.is_dir());
            }
            Err(err) => {
                panic!("expected vendor to succeed, but failed with: {}", err);
            }
        }
    }

    #[test]
    fn test_ensure_vendor_err_vendor_is_file() {
        let root = &tests::tempdir();
        let vendor = root.path().join("vendor");
        write_to(&vendor, "");

        match create_vendor_path(&vendor) {
            Ok(()) => {
                panic!("expected to fail, but succeeded with: {}", vendor.display());
            }
            Err(err) => {
                assert_eq!(
                    format!(
                        "vendor path '{}' already exists, and it's not a directory",
                        vendor.display()
                    ),
                    err.to_string()
                )
            }
        }
    }
}

//! # Finalizer module
//!
//! This module provide helpers methods to interact with kubernetes' resource
//! finalizer

use kube::Resource;

/// returns if there is the given finalizer on the resource
pub fn contains<T>(obj: &T, finalizer: &str) -> bool
where
    T: Resource,
{
    if let Some(finalizers) = &obj.meta().finalizers {
        finalizers.iter().any(|f| finalizer == f)
    } else {
        false
    }
}

/// add finalizer to the resource
pub fn add<T>(mut obj: T, finalizer: &str) -> T
where
    T: Resource,
{
    if (&obj.meta().finalizers).is_some() {
        if !contains(&obj, finalizer) {
            obj.meta_mut().finalizers.as_mut().map(|finalizers| {
                finalizers.push(finalizer.into());
                finalizers
            });
        }
    } else {
        obj.meta_mut().finalizers = Some(vec![finalizer.into()])
    }

    obj
}

/// remove finalizer from the resource
pub fn remove<T>(mut obj: T, finalizer: &str) -> T
where
    T: Resource,
{
    if let Some(finalizers) = &obj.meta().finalizers {
        obj.meta_mut().finalizers = Some(
            finalizers
                .iter()
                .filter(|f| *f != finalizer)
                .cloned()
                .collect(),
        );
    }

    obj
}

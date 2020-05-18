use crate::{
    update_asset_storage_system, AssetChannel, AssetLoader, AssetServer, ChannelAssetHandler,
    Handle, HandleId,
};
use bevy_app::{stage, AppBuilder, Events};
use bevy_core::bytes::GetBytes;
use legion::prelude::*;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub enum AssetEvent<T> {
    Created { handle: Handle<T> },
    Modified { handle: Handle<T> },
    Removed { handle: Handle<T> },
}

pub struct Assets<T> {
    assets: HashMap<Handle<T>, T>,
    paths: HashMap<PathBuf, Handle<T>>,
    events: Events<AssetEvent<T>>,
}

impl<T> Default for Assets<T> {
    fn default() -> Self {
        Assets {
            assets: HashMap::default(),
            paths: HashMap::default(),
            events: Events::default(),
        }
    }
}

impl<T> Assets<T> {
    pub fn get_with_path<P: AsRef<Path>>(&mut self, path: P) -> Option<Handle<T>> {
        self.paths.get(path.as_ref()).map(|handle| *handle)
    }

    pub fn add(&mut self, asset: T) -> Handle<T> {
        let handle = Handle::new();
        self.assets.insert(handle, asset);
        self.events.send(AssetEvent::Created { handle });
        handle
    }

    pub fn set(&mut self, handle: Handle<T>, asset: T) {
        let exists = self.assets.contains_key(&handle);
        self.assets.insert(handle, asset);

        if exists {
            self.events.send(AssetEvent::Modified { handle });
        } else {
            self.events.send(AssetEvent::Created { handle });
        }
    }

    pub fn add_default(&mut self, asset: T) -> Handle<T> {
        let handle = Handle::default();
        let exists = self.assets.contains_key(&handle);
        self.assets.insert(handle, asset);
        if exists {
            self.events.send(AssetEvent::Modified { handle });
        } else {
            self.events.send(AssetEvent::Created { handle });
        }
        handle
    }

    pub fn set_path<P: AsRef<Path>>(&mut self, handle: Handle<T>, path: P) {
        self.paths.insert(path.as_ref().to_owned(), handle);
    }

    pub fn get_id(&self, id: HandleId) -> Option<&T> {
        self.assets.get(&Handle::from_id(id))
    }

    pub fn get_id_mut(&mut self, id: HandleId) -> Option<&mut T> {
        self.assets.get_mut(&Handle::from_id(id))
    }

    pub fn get(&self, handle: &Handle<T>) -> Option<&T> {
        self.assets.get(&handle)
    }

    pub fn get_mut(&mut self, handle: &Handle<T>) -> Option<&mut T> {
        self.assets.get_mut(&handle)
    }

    pub fn get_or_insert_with(
        &mut self,
        handle: Handle<T>,
        insert_fn: impl FnOnce() -> T,
    ) -> &mut T {
        let mut event = None;
        let borrowed = self.assets.entry(handle).or_insert_with(|| {
            event = Some(AssetEvent::Created { handle });
            insert_fn()
        });

        if let Some(event) = event {
            self.events.send(event);
        }
        borrowed
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.assets.iter().map(|(k, v)| (*k, v))
    }

    pub fn remove(&mut self, handle: &Handle<T>) -> Option<T> {
        self.assets.remove(&handle)
    }

    pub fn asset_event_system(
        mut events: ResMut<Events<AssetEvent<T>>>,
        mut assets: ResMut<Assets<T>>,
    ) {
        events.extend(assets.events.drain())
    }
}

impl<T> GetBytes for Handle<T> {
    fn get_bytes(&self) -> Vec<u8> {
        Vec::new()
    }

    fn get_bytes_ref(&self) -> Option<&[u8]> {
        None
    }
}

pub trait AddAsset {
    fn add_asset<T>(&mut self) -> &mut Self
    where
        T: Send + Sync + 'static;
    fn add_asset_loader<TLoader, TAsset>(&mut self, loader: TLoader) -> &mut Self
    where
        TLoader: AssetLoader<TAsset> + Clone,
        TAsset: Send + Sync + 'static;
}

impl AddAsset for AppBuilder {
    fn add_asset<T>(&mut self) -> &mut Self
    where
        T: Send + Sync + 'static,
    {
        self.init_resource::<Assets<T>>()
            .add_system_to_stage(stage::POST_UPDATE, Assets::<T>::asset_event_system.system())
            .add_event::<AssetEvent<T>>()
    }

    fn add_asset_loader<TLoader, TAsset>(&mut self, loader: TLoader) -> &mut Self
    where
        TLoader: AssetLoader<TAsset> + Clone,
        TAsset: Send + Sync + 'static,
    {
        {
            if !self.resources().contains::<AssetChannel<TAsset>>() {
                self.resources_mut().insert(AssetChannel::<TAsset>::new());
                self.add_system_to_stage(
                    crate::stage::LOAD_ASSETS,
                    update_asset_storage_system::<TAsset>.system(),
                );
            }
            let asset_channel = self
                .resources()
                .get::<AssetChannel<TAsset>>()
                .expect("AssetChannel should always exist at this point.");
            let mut asset_server = self
                .resources()
                .get_mut::<AssetServer>()
                .expect("AssetServer does not exist. Consider adding it as a resource.");
            asset_server.add_loader(loader.clone());
            let handler = ChannelAssetHandler::new(loader, asset_channel.sender.clone());
            asset_server.add_handler(handler);
        }
        self
    }
}
#[macro_use]
extern crate rocket;

use core::panic;
use notify::{
    event::AccessKind, recommended_watcher, Event, EventKind, RecursiveMode, Result, Watcher,
};
use rocket::{futures::task::WakerRef, State};
use std::{
    borrow::BorrowMut,
    error::Error,
    ops::{Deref, DerefMut},
    path::Path,
    sync::{mpsc, Arc, RwLock},
    thread,
};
use wasmtime::{Engine, Instance, Module, Store};

// fn exec() -> Result<(), Box<dyn Error>> {
// An engine stores and configures global compilation settings like
// optimization level, enabled wasm features, etc.

// And finally we can call our function! Note that the error propagation
// with `?` is done to handle the case where the wasm function traps.
// let result = calc.call(&mut store, (2, 4))?;
// println!("calc: {:?}", result);
// Ok(())
// }

#[get("/")]
fn index(wasm_state: &State<Arc<RwLock<WasmState>>>) -> String {
    let val: u64 = {
        let wasm_mut = wasm_state.read().unwrap();
        let mut store_lock = wasm_mut.store.write().unwrap();
        wasm_mut
            .calc
            .call(&mut store_lock.deref_mut(), (2, 4))
            .unwrap()
    };
    let res = val.to_string();
    res
}

struct WasmState {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    store: RwLock<wasmtime::Store<()>>,
    instance: wasmtime::Instance,
    calc: wasmtime::TypedFunc<(u64, u64), u64>,
}

impl WasmState {
    pub fn new() -> Self {
        let engine = Engine::default();
        let module = Module::from_file(&engine, "calc.wasm").unwrap();
        let store = RwLock::new(Store::new(&engine, ()));
        let instance = {
            let mut store_lock = store.write().unwrap();
            Instance::new(&mut store_lock.deref_mut(), &module, &[]).unwrap()
        };
        let calc = {
            let mut store_lock = store.write().unwrap();
            instance
                .get_func(&mut store_lock.deref_mut(), "calc")
                .expect("`calc` was not an exported function")
        };
        let calc = {
            let store_lock = store.read().unwrap();
            calc.typed::<(u64, u64), u64>(&store_lock.deref()).unwrap()
        };
        WasmState {
            engine,
            module,
            store,
            instance,
            calc,
        }
    }

    pub fn reload(&mut self) {
        let module = Module::from_file(&self.engine, "calc.wasm").unwrap();
        let instance = {
            let mut store_lock = self.store.write().unwrap();
            Instance::new(&mut store_lock.deref_mut(), &module, &[]).unwrap()
        };
        let calc = {
            let mut store_lock = self.store.write().unwrap();
            instance
                .get_func(&mut store_lock.deref_mut(), "calc")
                .expect("`calc` was not an exported function")
        };
        let calc = {
            let store_lock = self.store.read().unwrap();
            calc.typed::<(u64, u64), u64>(&store_lock.deref()).unwrap()
        };
        self.module = module;
        self.instance = instance;
        self.calc = calc;
    }
}

#[launch]
fn rocket() -> _ {
    // Charge le module wasm et tout ce qui faut
    let wasm_state = Arc::new(RwLock::new(WasmState::new()));

    // Setup le thread qui va automatiquement recharger le module.
    let wasm_cloned = wasm_state.clone();
    thread::spawn(move || {
        println!("Start watcher.");
        let wasm = wasm_cloned;
        let (tx, rx) = mpsc::channel::<Result<Event>>();
        let mut watcher = notify::recommended_watcher(tx).unwrap();
        watcher
            .watch(Path::new("./calc.wasm"), RecursiveMode::Recursive)
            .unwrap();
        for res in rx {
            match res {
                Ok(event) => {
                    println!("event: {:?}", event);
                    match event.kind {
                        EventKind::Access(AccessKind::Close(_)) => {
                            println!("Reload WASM module.");
                            let mut wasm_mut = wasm.write().unwrap();
                            wasm_mut.reload();
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    println!("watch error: {:?}", e);
                    panic!("Error while watching.")
                }
            }
        }
    });

    rocket::build()
        .manage(wasm_state)
        .mount("/", routes![index])
}

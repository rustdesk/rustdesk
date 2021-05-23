/*! Sciter Object Model (SOM passport).

See [Native code exposure to script](http://sciter.com/native-code-exposure-to-script/)
and [Sciter Object Model](http://sciter.com/developers/for-native-gui-programmers/sciter-object-model/) blog articles.

*/
use std::sync::atomic::{AtomicI32, Ordering};
use capi::sctypes::{LPVOID, LPCSTR};
pub use capi::scom::*;


/// Get the index of an interned string.
pub fn atom(name: &str) -> som_atom_t {
	let s = s2u!(name);
	(crate::_API.SciterAtomValue)(s.as_ptr())
}

/// Get the value of an interned string.
pub fn atom_name(id: som_atom_t) -> Option<String> {
	let mut s = String::new();
	let ok = (crate::_API.SciterAtomNameCB)(id, crate::utf::store_astr, &mut s as *mut _ as LPVOID);
	if ok != 0 {
		Some(s)
	} else {
		None
	}
}


/// Something that has a SOM passport.
///
/// However, since we can't call extern functions in static object initialization,
/// in order to use [`atom("name")`](fn.atom.html) we have to initializa the passport in run time
/// and return a reference to it via [`Box::leak()`](https://doc.rust-lang.org/stable/std/boxed/struct.Box.html#method.leak).
pub trait Passport {
	/// A static reference to the passport that describes an asset.
	fn get_passport(&self) -> &'static som_passport_t;
}


/// A non-owning pointer to a native object.
pub struct IAssetRef<T> {
	asset: *mut som_asset_t,
	ty: std::marker::PhantomData<T>,
}

impl<T> Clone for IAssetRef<T> {
	fn clone(&self) -> Self {
		self.add_ref();
		Self {
			asset: self.asset,
			ty: self.ty,
		}
	}
}

impl<T> Drop for IAssetRef<T> {
	fn drop(&mut self) {
		self.release();
	}
}

impl<T> std::fmt::Debug for IAssetRef<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		// the current reference count
		self.add_ref();
		let rc = self.release();

		let name = self::atom_name(self.get_passport().name);
		write!(f, "Asset({}):{}", name.unwrap_or_default(), rc)
	}
}

/// Construct a reference from a managed asset.
impl<T> From<Box<IAsset<T>>> for IAssetRef<T> {
	fn from(asset: Box<IAsset<T>>) -> Self {
		Self::from_raw(IAsset::into_raw(asset))
	}
}

impl<T> IAssetRef<T> {
	/// Get the vtable of an asset.
	fn isa(&self) -> &'static som_asset_class_t {
		unsafe { (*self.asset).isa }
	}

	/// Increment the reference count of an asset and returns the new value.
	fn add_ref(&self) -> i32 {
		(self.isa().add_ref)(self.asset)
	}

	/// Decrement the reference count of an asset and returns the new value.
	fn release(&self) -> i32 {
		(self.isa().release)(self.asset)
	}
}

impl<T> IAssetRef<T> {
	/// Construct from a raw pointer, incrementing the reference count.
	pub fn from_raw(asset: *mut som_asset_t) -> Self {
		eprintln!("IAssetRef<{}>::from({:?})", std::any::type_name::<T>(), asset);
		assert!(asset.is_null() == false);
		let me = Self {
			asset,
			ty: std::marker::PhantomData,
		};
		me.add_ref();
		me
	}

	/// Return the raw pointer, releasing the reference count.
	pub fn into_raw(asset: IAssetRef<T>) -> *mut som_asset_t {
		// decrement reference count
		asset.release();

		// get the pointer and forget about this wrapper
		let ptr = asset.asset;
		std::mem::forget(asset);

		ptr
	}

	/// Get the underlaying pointer.
	pub fn as_ptr(&self) -> *mut som_asset_t {
		self.asset
	}

	/// Get a reference to the underlaying pointer.
	#[doc(hidden)]
	pub fn as_ref(&self) -> &som_asset_t {
		// TODO: do we need this?
		unsafe { & *self.asset }
	}

	/// Get the passport of the asset.
	pub fn get_passport(&self) -> &som_passport_t {
		// TODO: do we need this?
		let ptr = (self.isa().get_passport)(self.asset);
		unsafe { & *ptr }
	}
}


/// An owned pointer to a wrapped native object.
#[repr(C)]
pub struct IAsset<T> {
	// NB: should be the first member here
	// in order to `*mut IAsset as *mut som_asset_t` work
	asset: som_asset_t,
	refc: AtomicI32,
	passport: Option<&'static som_passport_t>,
	data: T,
}

/// Make the object to be accessible as other global objects in TIScript.
pub fn set_global<T>(asset: IAssetRef<T>) {
	let ptr = asset.as_ptr();
	// eprintln!("IAsset<{}>: {:?}", std::any::type_name::<T>(), ptr);
	(crate::_API.SciterSetGlobalAsset)(ptr);
}

/// Make the object to be accessible as other global objects in TIScript.
pub fn into_global<T>(asset: Box<IAsset<T>>) {
	let ptr = IAsset::into_raw(asset);
	// eprintln!("IAsset<{}>: {:?}", std::any::type_name::<T>(), ptr);
	(crate::_API.SciterSetGlobalAsset)(ptr);
}

impl<T> std::ops::Deref for IAsset<T> {
	type Target = T;
	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<T> std::ops::DerefMut for IAsset<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.data
	}
}

impl<T> Drop for IAsset<T> {
	fn drop(&mut self) {
		let rc = self.refc.load(Ordering::SeqCst);
		if rc != 0 {
			eprintln!("asset<{}>::drop with {} references alive", std::any::type_name::<T>(), rc);
		}
		assert_eq!(rc, 0);
		// allocated in `iasset::new()`
		let ptr = self.asset.isa as *const som_asset_class_t;
		let ptr = unsafe { Box::from_raw(ptr as *mut som_asset_class_t) };
		drop(ptr);
	}
}

impl<T> IAsset<T> {
	/// Cast the pointer to a managed asset reference.
	pub fn from_raw(thing: &*mut som_asset_t) -> &mut IAsset<T> {
		assert!(thing.is_null() == false);
		unsafe { &mut *(*thing as *mut IAsset<T>) }
	}

	/// Release the pointer.
	fn into_raw(asset: Box<IAsset<T>>) -> *mut som_asset_t {
		let p = Box::into_raw(asset);
		p as *mut som_asset_t
	}
}

impl<T: Passport> IAsset<T> {
	/// Wrap the object into a managed asset.
	pub fn new(data: T) -> Box<Self> {
		// will be freed in `iasset<T>::drop()`
		let isa = Box::new(Self::class());

		let me = Self {
			asset: som_asset_t { isa: Box::leak(isa) },
			refc: Default::default(),
			passport: None,
			data,
		};
		Box::new(me)
	}

	fn class() -> som_asset_class_t {
		extern "C" fn asset_add_ref<T>(thing: *mut som_asset_t) -> i32 {
			{
				let me = IAsset::<T>::from_raw(&thing);
				let t = me.refc.fetch_add(1, Ordering::SeqCst) + 1;
				// eprintln!("iasset<T>::add_ref() -> {}", t);
				return t;
			}
		}
		extern "C" fn asset_release<T>(thing: *mut som_asset_t) -> i32 {
			let t = {
				let me = IAsset::<T>::from_raw(&thing);
				me.refc.fetch_sub(1, Ordering::SeqCst) - 1
			};
			// eprintln!("iasset<T>::release() -> {}", t);
			if t == 0 {
				// eprintln!("iasset<T>::drop()");
				let me = unsafe { Box::from_raw(thing as *mut IAsset<T>) };
				drop(me);
			}
			return t;
		}
		extern "C" fn asset_get_interface<T>(_thing: *mut som_asset_t, name: LPCSTR, _out: *mut *mut som_asset_t) -> bool {
			let name = u2s!(name);
			eprintln!("iasset<T>::get_interface({}) is not implemented.", name);
			return false;
		}
		extern "C" fn asset_get_passport<T: Passport>(thing: *mut som_asset_t) -> *const som_passport_t
		{
			// here we cache the returned reference in order not to allocate things again
			let me = IAsset::<T>::from_raw(&thing);
			if me.passport.is_none() {
				// eprintln!("asset_get_passport<{}>: {:?}", std::any::type_name::<T>(), thing);
				me.passport = Some(me.data.get_passport());
			}
			let ps = me.passport.as_ref().unwrap();
			return *ps;
		}

		som_asset_class_t {
			add_ref: asset_add_ref::<T>,
			release: asset_release::<T>,
			get_interface: asset_get_interface::<T>,
			get_passport: asset_get_passport::<T>,
		}
	}
}

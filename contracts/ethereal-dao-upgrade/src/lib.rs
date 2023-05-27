use scrypto::prelude::*;

// constant to avoid any funny business
// how-to: radix-engine-toolkit address decode --address "package_[...]" or "component_[...]"
// copy data

// current: RCNet edao 2
const NEW_ADDR: &str = "0086a31ef8c50dfe7aedc2944551a37f699f408f178e041c1affeb";

const NEW_MODULE: &str = "DAO"; // module name to call the above package with

// current: RCNet edao 1
const OLD_ADDR: &str = "03b83843328ec11a234d2a291276432c2e9512616a547eda9490e2"; 

// using static dispatch is preferrable as both are known statically
external_component! {
  Dao1 {
    fn to_nothing(&mut self) -> Bucket;
    fn look_within(&self) -> (
      ResourceAddress, 
      HashMap<
        ResourceAddress, 
        Vec<u64>>,
      u64,
      ResourceAddress,
      ResourceAddress);
  }
}

external_blueprint! {
  Dao2 {
    fn from_something(ds: Bucket, 
      pm: HashMap<
        ResourceAddress, 
        Vec<u64>>,
      did: u64,
      pz: ResourceAddress,
      po: ResourceAddress) -> ComponentAddress;
  }
}

// this entire script is based on a *specific* version
// of the ethereal-dao script
// i.e. it is the patchwork needed to move from *this exact one*
// to *that exact one*. with any changes needed to make the update happen
// including any and all hacks
#[blueprint]
mod daoup {
  struct DAOUP {
    // badge holding permisssions over the removal 
    // of the superbadge
    vault_zero: Vault,
  }

  // the reason why there's two functions instead of one is that 
  // funny things could happen if white Dao1 is calling f its soul gets ripped out
  // potentially able to be merged into one
  impl DAOUP {
    pub fn the_dao_is_dead(zero: Bucket) -> ComponentAddress {

      // sanity check that they do indeed deserialize
      let _dao1 = ComponentAddress::try_from_hex(OLD_ADDR).unwrap();
      let _dao2 = PackageAddress::try_from_hex(NEW_ADDR).unwrap();

      Self {
        vault_zero: Vault::with_bucket(zero)
      }
      .instantiate()
      .globalize()
    }

    // d1 and d2 need to be passed in as arguments 
    // or get cucked by CallFrameError(RENodeNotVisible .. )) error
    pub fn long_live_the_dao(&mut self, d1: ComponentAddress, d2: PackageAddress) -> ComponentAddress {
      
      let dao1 = ComponentAddress::try_from_hex(OLD_ADDR).unwrap();
      let dao2 = PackageAddress::try_from_hex(NEW_ADDR).unwrap();
      info!("passed unwraps");

      assert!( d1 == dao1 && d2 == dao2, "wrong addresses");

      let dao_superbadge: Bucket = 
        self.vault_zero.authorize(||
          Dao1::at(dao1).to_nothing()
        );

      info!("passed call 1");
      let (ds, pm, did, pz, po) = Dao1::at(dao1).look_within();
      info!("passed call 2");

      // kind of a useless assertion
      assert!( ds == dao_superbadge.resource_address(), "incoherence of dao souls");

      Dao2::at(dao2, NEW_MODULE).from_something(dao_superbadge, pm, did, pz, po)
    }

  }
}
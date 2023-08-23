use scrypto::prelude::*;
use std::ops::DerefMut;

#[blueprint]
mod delta {
  enable_method_auth! {
    roles {
      zero => updatable_by: [];
      alpha => updatable_by: [];
    },
    methods {
      to_nothing => restrict_to: [zero];
      aa_tap => restrict_to: [alpha];
      aa_out => restrict_to: [alpha];
      deposit => PUBLIC;
    }
  }

  
  struct Delta {
    dao_addr: ComponentAddress,
    power_delta: Vault,

    // (REAL, EUXLP), that will be used for AA
    // REAL (to be paired with EUXLP)
    // EUXLP (stored, if ever REAL is depleted)
    aa_treasury: (Vault, Vault),
    // doubles down as a whitelist and approved spending
    treasury: KeyValueStore<ResourceAddress, (Decimal, Vault)>,
  }

  impl Delta {
    pub fn from_nothing(dao_addr: ComponentAddress, power_zero: ResourceAddress,
      power_alpha: ResourceAddress, power_delta: Bucket, whitelist: Vec<(ResourceAddress, Decimal)>,
      real:Bucket, euxlp: ResourceAddress
    ) -> ComponentAddress {
      // needs to whitelist
      // real, tlp, euxlp, exrd, xrd, eusd

      let aa_treasury = (Vault::with_bucket(real), Vault::new(euxlp));

      let treasury = KeyValueStore::new();
      for (ra, d) in whitelist {
        treasury.insert(ra, (d, Vault::new(ra)));
      }

      Self {
        dao_addr,
        power_delta: Vault::with_bucket(power_delta),

        aa_treasury,
        treasury
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          alpha => rule!(require(power_alpha));
          zero => rule!(require(power_zero));
        )
      )
      .globalize()
      .address()
    }

    pub fn deposit(&mut self, input: Bucket) {
      match self.treasury.get_mut(&input.resource_address()) {
        None => panic!("non whitelist deposit type"),
        Some(mut v) => v.deref_mut().1.put(input)
      }
    }

    // GIVE ME ALL OF THE REAL
    // and, if you've any EUXLP, that also
    pub fn aa_tap(&mut self) -> (Bucket, Option<Bucket>) {
      // honestly it pulling all at once is a hack to add miaximum possible size
      // without doing any calculation
      (self.aa_treasury.0.take_all(), 
        if self.aa_treasury.1.is_empty() { None } else { Some(self.aa_treasury.1.take_all()) }
      )
    }

    // thank you for being real with me
    pub fn aa_out(&mut self, ret: Option<Bucket>) {
      if let Some(r) = ret {
        if r.resource_address() == self.aa_treasury.0.resource_address() {
          self.aa_treasury.0.put(r);
        } else {
          self.aa_treasury.1.put(r);
        }
      }
    }

    pub fn to_nothing(&mut self, ) {

    }

    // todo move from treasury to aa treasury

    // internal 

    // fn authorize<F: FnOnce() -> O, O>(power: &mut Vault, f: F) -> O {
    //   let temp = power.as_fungible().take_all();
    //   let ret = temp.authorize_with_all(|| {
    //     f()
    //   });
    //   power.put(temp.into());
    //   return ret
    // }
  }
}
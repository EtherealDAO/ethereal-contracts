use scrypto::prelude::*;

// let component: Global<AnyComponent> = Global(ObjectStub::new(ObjectStubHandle::Global(GlobalAddress::from(component_address))));
// let return_value = component.call_raw::<ZygomebFancyReturnType>("method_name", scrypto_args!(param1));

#[blueprint]
mod alpha {
  enable_method_auth! {
    roles {
      zero => updatable_by: [];
      // usd => updatable_by: []; TODO RESTRICT
    },
    methods {
      to_nothing => restrict_to: [zero];
      aa_rope => PUBLIC; // TODO restrict?
      set_app_addrs => restrict_to: [zero];
      get_app_addrs => PUBLIC;
    }
  }

  // static-participant multisig 
  // self-governed via 3/3, each participant being a DAO branch
  struct Alpha {
    dao_addr: ComponentAddress,
    power_zero: ResourceAddress,

    power_alpha: Vault,
    power_azero: ResourceAddress, // alpha zero, zero of alpha
    
    // usd, eux, tri
    app_addrs: (ComponentAddress, ComponentAddress, ComponentAddress),
  }

  impl Alpha {
    pub fn from_nothing(
      dao_addr: ComponentAddress, power_zero: ResourceAddress, 
      power_alpha: Bucket, power_azero: ResourceAddress,
      usd_addr: ComponentAddress, eux_addr: ComponentAddress, tri_addr: ComponentAddress,
    ) -> ComponentAddress {
      // power azero is passed in
      // dao script is deferred to for all the braiding
      // despite the layers being one step down the same, really
      

      Self {
        dao_addr,
        power_zero,

        power_alpha: Vault::with_bucket(power_alpha),
        power_azero,

        app_addrs: (usd_addr, eux_addr, tri_addr),
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          zero => rule!(require(power_zero));
        )
      )
      .globalize()
      .address()
    }

    pub fn to_nothing(&mut self) {

    }

    // TODO: auth? is it worth guarding against someone 
    // donating their EUXLP here? like it makes treasury add liquidity 
    // but is that a bad thing? coung be an add high type situation
    // that makes the treasure take an L on the real it holds
    // but equivalently they can probably just swap
    // and it would be just as effective, if done with side that moves liquidity
    //
    // honestly don't see it being a problem: TODO ask vex
    //
    // automatically pairs it with treasury REAL
    pub fn aa_rope(&mut self, mut input: Bucket) {
      // no check if it's euxlp, but if it isn't, it explodes HERE
      let dao: Global<AnyComponent> = self.dao_addr.into();

      let (_, delta_ca, _) = 
        dao.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>(
          "get_branch_addrs", scrypto_args!()
        );
      let delta: Global<AnyComponent> = delta_ca.into();
      
      // token boosted POL acquisition
      let real = Self::authorize(&mut self.power_alpha, || { 
        let (real, rem) = delta.call_raw::<(Bucket, Option<Bucket>)>
          ("aa_tap", scrypto_args!());
        
        if let Some(r) = rem {
          input.put(r);
        };

        real
      });

      // assumes order of REAL / EUXLP
      // HERE
      let tri: Global<AnyComponent> = self.app_addrs.2.into();
      let (tlp, remainder) = 
        tri.call_raw::<(Bucket, Option<Bucket>)>("add_liquidity", scrypto_args!(real, input));

      Self::authorize(&mut self.power_alpha, || { 
        delta.call_raw::<()>
          ("aa_out", scrypto_args!(remainder));
        delta.call_raw::<()>
          ("deposit", scrypto_args!(tlp));
      });
    }

    pub fn get_app_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress) {
      self.app_addrs
    }

    pub fn set_app_addrs(&mut self, new: (ComponentAddress, ComponentAddress, ComponentAddress)) {
      self.app_addrs = new;
    }


    // internal 

    fn authorize<F: FnOnce() -> O, O>(power: &mut Vault, f: F) -> O {
      let temp = power.as_fungible().take_all();
      let ret = temp.authorize_with_all(|| {
        f()
      });
      power.put(temp.into());
      return ret
    }

  }
}
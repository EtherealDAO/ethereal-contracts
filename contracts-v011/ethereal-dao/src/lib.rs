use scrypto::prelude::*;

// ZERO-TH DAO
// DELPOYS EVERYTHING, AND THEN IS REBORN ANEW
#[blueprint]
mod dao {
  enable_method_auth! {
    roles {
      zero => updatable_by: [];
    },
    methods {
      from_nothing_er => PUBLIC;
      set_branch_addrs => restrict_to: [zero];
      get_branch_addrs => PUBLIC;
      set_phase2_args => restrict_to: [zero];
    }
  }

  struct Dao {
    // deploy phase
    phase: u64,

    power_dao: Vault,
    souls: (ResourceAddress, ResourceAddress, ResourceAddress),
    power_zero: ResourceAddress,

    // alpha, Delta, omega
    branch_addrs: (ComponentAddress, ComponentAddress, ComponentAddress),

    // phase 2 variables 
    tri_p: PackageAddress,
    power_azero: ResourceAddress,
    power_tri :Vault,
    exrd: ResourceAddress,

    power_delta: Vault,
    delta_p: PackageAddress,
    delta_whitelist: Vec<(ResourceAddress, Decimal)>,
    real: Vault,
    euxlp: ResourceAddress
  }

  impl Dao {
    pub fn from_nothing( // todo Omega
      alpha_p: PackageAddress, delta_p: PackageAddress,
      usd_p: PackageAddress, eux_p: PackageAddress, tri_p: PackageAddress,
      real: Bucket, exrd: ResourceAddress, bang: ComponentAddress
      ) -> (ComponentAddress, Bucket) {
      // todo for now just a mock script helping the setup/reproducible redeploy
      // + addr beacon

      let u_lower = dec!("0.99");
      let u_upper = dec!("1.01");
      let u_flash_fee = dec!("1.001");
      let u_mock_oracle = dec!("1");

      let e_swap_fee = dec!("0.997");
      
      let power_dao = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);
      let mut power_alpha = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(2); // todo temp hack
      let power_delta = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);
      // let power_omega = ResourceBuilder::new_fungible(OwnerRole::None)
      //   .mint_initial_supply(1);

      let power_usd = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);
      let power_eux = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);
      let power_tri = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);

      let power_zero = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_roles(mint_roles!(
          minter => rule!(require(power_dao.resource_address()));
          minter_updater => rule!(deny_all);
        ))
        .burn_roles(burn_roles!(
          burner => rule!(allow_all);
          burner_updater => rule!(deny_all);
        ))
        .create_with_no_initial_supply();

      let power_azero = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_roles(mint_roles!(
          minter => rule!(require(power_alpha.resource_address()));
          minter_updater => rule!(deny_all);
        ))
        .burn_roles(burn_roles!(
          burner => rule!(allow_all);
          burner_updater => rule!(deny_all);
        ))
        .create_with_no_initial_supply();
      
      let the_zero = power_dao.as_fungible().authorize_with_all(|| 
        ResourceManager::from(power_zero).mint(1)
      );

      let real_resource = real.resource_address();

      let dao_addr = Self {
        phase: 1u64,
        power_dao: Vault::with_bucket(power_dao.into()),
        souls: (
          power_alpha.resource_address(), 
          power_delta.resource_address(), 
          power_alpha.resource_address() // TODO OMEGA
        ),
        power_zero: power_zero.address(),

        branch_addrs: (bang, bang, bang),

        // phase 2
        tri_p,
        power_azero: power_azero.address(),
        power_tri: Vault::with_bucket(power_tri.into()),
        exrd,

        power_delta: Vault::with_bucket(power_delta.into()),
        delta_p,
        delta_whitelist: vec![],
        real: Vault::with_bucket(real),
        euxlp: power_zero.address()
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          zero => rule!(require(power_zero.address()));
        )
      )
      .globalize()
      .address();

      let alpha_resource = power_alpha.resource_address();
      // let _omega_resource = power_omega.resource_address();

      let out = ScryptoVmV1Api::blueprint_call(
            alpha_p,
            "Alpha",
            "from_nothing",
            scrypto_args!(
              dao_addr, power_zero,
              power_alpha.take(dec!(1)), power_azero,
              bang, bang, bang
            )
        );
      let alpha_addr: ComponentAddress = scrypto_decode(&out).unwrap();

      // let ap: Global<PackageStub> = alpha_p.into();
      // let alpha_addr = ap.call_raw::<ComponentAddress>(
      //   "from_nothing", scrypto_args!(
      //     dao_addr, power_zero,
      //     power_alpha.take(dec!(1)), power_azero,
      //     bang, bang, bang
      //   )
      // );
  
      let out = ScryptoVmV1Api::blueprint_call(
            usd_p,
            "Usd",
            "from_nothing",
            scrypto_args!(
              alpha_addr, alpha_resource,
              power_eux.resource_address().clone(), power_usd,
              exrd, u_lower, u_upper, u_flash_fee, u_mock_oracle
            )
        );
      let (usd_addr, eusd_resource): (ComponentAddress, ResourceAddress) = 
        scrypto_decode(&out).unwrap();

      // let up: Global<PackageStub> = usd_p.into();
      // let (usd_addr, eusd_resource) = up.call_raw::<(ComponentAddress, ResourceAddress)>(
      //   "from_nothing", scrypto_args!(
      //     dao_addr, alpha_resource,
      //     power_eux.resource_address().clone(), power_usd,
      //     exrd, u_lower, u_upper, u_flash_fee, u_mock_oracle
      //   )
      // );

      let out = ScryptoVmV1Api::blueprint_call(
            eux_p,
            "Eux",
            "from_nothing",
            scrypto_args!(
              alpha_addr, alpha_resource, power_azero,
              power_eux, eusd_resource, exrd, e_swap_fee
            )
        );
      let (eux_addr, euxlp_resource): (ComponentAddress, ResourceAddress) = 
        scrypto_decode(&out).unwrap();
      
      // let ep: Global<PackageStub> = eux_p.into();
      // let (eux_addr, euxlp_resource) = ep.call_raw::<(ComponentAddress, ResourceAddress)>(
      //   "from_nothing", scrypto_args!(
      //     alpha_addr, alpha_resource, power_azero,
      //     power_eux, eusd_resource, exrd, e_swap_fee
      //   )
      // );

      

      // let tp: Global<PackageStub> = tri_p.into();
      // let (tri_addr, tlp_resource) = tp.call_raw::<(ComponentAddress, ResourceAddress)>(
      //   "from_nothing", scrypto_args!(
      //     alpha_addr, alpha_resource, power_azero,
      //     power_tri,
      //     real.resource_address(), t_w1,
      //     exrd, t_w2,
      //     t_swap_fee
      //   )
      // );

      the_zero.as_fungible().authorize_with_all(|| {
        let alpha: Global<AnyComponent> = alpha_addr.into();
        alpha.call_raw::<()>(
          "set_app_addrs", scrypto_args!((usd_addr, eux_addr, bang))
        );
        let dao: Global<AnyComponent> = dao_addr.into();
        dao.call_raw::<()>(
          "set_phase2_args", scrypto_args!(
            vec![
            (XRD, dec!(0)), (real_resource, dec!(0)), 
            (eusd_resource, dec!(0)), (euxlp_resource, dec!(0)),
            (exrd, dec!(0))
            ],
            euxlp_resource
          )
        );
        dao.call_raw::<()>(
          "set_branch_addrs", scrypto_args!((alpha_addr, bang, bang)) // TOOD
        )
      });

      the_zero.burn();
    
      // todo remove alpha
      (dao_addr, power_alpha.into())
    }

    // deploy second part
    pub fn from_nothing_er(&mut self) {
      assert!( 1u64 == self.phase, 
       "out of order call");
      
      self.phase += 1;
      let the_zero = Self::authorize(&mut self.power_dao, || 
        ResourceManager::from(self.power_zero).mint(1)
      );

      let dao_addr = Runtime::global_address();
      let (alpha_addr, _, _) = self.branch_addrs;
      let (alpha_resource, _, _) = self.souls;

      let t_w1 = dec!("0.90");
      let t_w2 = dec!("0.10");
      let t_swap_fee = dec!("0.997");

      let out = ScryptoVmV1Api::blueprint_call(
            self.tri_p,
            "Tri",
            "from_nothing",
            scrypto_args!(
              alpha_addr, alpha_resource, self.power_azero,
              self.power_tri.take_all(),
              self.real.resource_address(), t_w1,
              self.euxlp, t_w2,
              t_swap_fee
            )
        );
      let (tri_addr, tlp_resource): (ComponentAddress, ResourceAddress) = 
        scrypto_decode(&out).unwrap();

      self.delta_whitelist.push((tlp_resource, dec!(0)));
      let out = ScryptoVmV1Api::blueprint_call(
            self.delta_p,
            "Delta",
            "from_nothing",
            scrypto_args!(
              dao_addr, self.power_zero,
              alpha_resource, self.power_delta.take_all(),
              &self.delta_whitelist,
              self.real.take_all(), // TODO for now drops ALL real into AA use
              self.euxlp
            )
        );
      let delta_addr: ComponentAddress = 
        scrypto_decode(&out).unwrap();

      // let dp: Global<PackageStub> = delta_p.into();
      // let delta_addr = dp.call_raw::<ComponentAddress>(
      //   "from_nothing", scrypto_args!(
      //     dao_addr, power_zero,
      //     alpha_resource, power_delta,
      //     vec![
      //       (XRD, dec!(0)), (real.resource_address(), dec!(0)), 
      //       (eusd_resource, dec!(0)), (euxlp_resource, dec!(0)),
      //       (exrd, dec!(0)), (tlp_resource, dec!(0))],
      //     real, // TODO for now drops ALL real into AA use
      //     euxlp_resource
      //   )
      // );

      // TODO omega

      let bang = alpha_addr;
      self.set_branch_addrs((alpha_addr, delta_addr, bang)); // TODO

      the_zero.as_fungible().authorize_with_all(|| {
        let alpha: Global<AnyComponent> = alpha_addr.into();
        let (usd_addr, eux_addr, _) = alpha.call_raw
          ::<(ComponentAddress, ComponentAddress, ComponentAddress)>(
          "get_app_addrs", scrypto_args!()
        ); 
        alpha.call_raw::<()>(
          "set_app_addrs", scrypto_args!((usd_addr, eux_addr, tri_addr))
        );
      });

      the_zero.burn();
    }

    pub fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress) {
      self.branch_addrs
    }

    pub fn set_branch_addrs(&mut self, new: (ComponentAddress, ComponentAddress, ComponentAddress)) {
      self.branch_addrs = new;
    }

    // phase-braiding functions

    pub fn set_phase2_args(&mut self, wl: Vec<(ResourceAddress, Decimal)>, 
      euxlp: ResourceAddress) {
      self.delta_whitelist = wl;
      self.euxlp = euxlp;
    }
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
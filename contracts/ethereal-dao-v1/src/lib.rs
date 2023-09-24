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
    power_tri: Vault,
    exrd: ResourceAddress,

    power_delta: Vault,
    delta_p: PackageAddress,
    real: Vault,
    euxlp: ResourceAddress,
    bang: ComponentAddress,

    omega_p: PackageAddress,
    power_omega: Vault,

    daov2_p: PackageAddress
  }

  impl Dao {
    // bang is the dapp definition addr
    // also used as dummy addr be4 braiding
    pub fn from_nothing( // todo Omega
      alpha_p: PackageAddress, delta_p: PackageAddress, omega_p: PackageAddress,
      usd_p: PackageAddress, eux_p: PackageAddress, tri_p: PackageAddress, daov2_p: PackageAddress,
      real: Bucket, exrd: ResourceAddress, exrd_validator: ComponentAddress, bang: ComponentAddress
      ) -> (ComponentAddress, Bucket, Bucket) {

      let u_lower = dec!("0.99");
      let u_upper = dec!("1.01");
      let u_flash_fee = dec!("1.001");
      let u_init_oracle = dec!("1");

      let e_swap_fee = dec!("0.997");
      
      let power_dao = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              // that which creates all, cannot belong to it
              metadata_setter => rule!(deny_all);
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "name" => "POWER DAO", locked;
            }
          )
        )
        .mint_initial_supply(1);

      let power_zero = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_dao.resource_address()));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER ZERO", locked;
            }
          )
        )
        .mint_roles(mint_roles!(
          minter => rule!(require(power_dao.resource_address()));
          minter_updater => rule!(deny_all);
        ))
        .burn_roles(burn_roles!(
          burner => rule!(allow_all);
          burner_updater => rule!(deny_all);
        ))
        .create_with_no_initial_supply()
        .address();

      let power_alpha = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER ALPHA", locked;
            }
          )
        )
        .mint_initial_supply(1);

      let power_delta = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER DELTA", locked;
            }
          )
        )
        .mint_initial_supply(1);
      let power_omega = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER OMEGA", locked;
            }
          )
        )
        .mint_initial_supply(1);

      let power_usd = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER USD", locked;
            }
          )
        )
        .mint_initial_supply(1);

      let oracle1 = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "EDAO PRIMARY ORACLE", locked;
            }
          )
        )
        .mint_initial_supply(1);

      let oracle2 = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "EDAO BACKUP ORACLE", locked;
            }
          )
        )
        .mint_initial_supply(1);

      let power_eux = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER EUX", locked;
            }
          )
        )
        .mint_initial_supply(1);
      let power_tri = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER TRI", locked;
            }
          )
        )
        .mint_initial_supply(1);

      let power_azero = ResourceBuilder::new_fungible(OwnerRole::None)
        .metadata(
          metadata!(
            roles {
              metadata_setter => rule!(require(power_zero));
              metadata_setter_updater => rule!(deny_all);
              metadata_locker => rule!(deny_all);
              metadata_locker_updater => rule!(deny_all);
            },
            init {
              "dapp_definition" =>
                GlobalAddress::from(bang), updatable;
              "name" => "POWER ALPHA ZERO", locked;
            }
          )
        )
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

      let omega_resource = power_omega.resource_address();

      let dao_addr = Self {
        phase: 1u64,
        power_dao: Vault::with_bucket(power_dao.into()),
        souls: (
          power_alpha.resource_address(), 
          power_delta.resource_address(), 
          power_omega.resource_address()
        ),
        power_zero: power_zero,

        branch_addrs: (bang, bang, bang),

        // phase 2
        tri_p,
        power_azero: power_azero.address(),
        power_tri: Vault::with_bucket(power_tri.into()),
        exrd,

        power_delta: Vault::with_bucket(power_delta.into()),
        delta_p,
        real: Vault::with_bucket(real),
        euxlp: power_zero,
        bang,

        omega_p,
        power_omega: Vault::with_bucket(power_omega.into()),

        daov2_p
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .roles(
        roles!(
          zero => rule!(require(power_zero));
        )
      )
      .metadata(
        metadata!(
          roles {
            metadata_setter => rule!(require(power_zero));
            metadata_setter_updater => rule!(deny_all);
            metadata_locker => rule!(deny_all);
            metadata_locker_updater => rule!(deny_all);
          },
          init {
            "dapp_definition" =>
              GlobalAddress::from(bang), updatable;
          }
        )
      )
      .globalize()
      .address();

      let out = ScryptoVmV1Api::blueprint_call(
            alpha_p,
            "Alpha",
            "from_nothing",
            scrypto_args!(
              dao_addr, power_zero,
              omega_resource, power_alpha, power_azero,
              bang, bang, bang, bang
            )
        );
      let alpha_addr: ComponentAddress = scrypto_decode(&out).unwrap();
  
      let out = ScryptoVmV1Api::blueprint_call(
            usd_p,
            "Usd",
            "from_nothing",
            scrypto_args!(
              alpha_addr, power_azero,
              power_eux.resource_address().clone(), power_usd,
              exrd, exrd_validator, u_lower, u_upper, u_flash_fee, bang, 
              u_init_oracle, oracle1.resource_address(), oracle2.resource_address()
            )
        );
      let (usd_addr, eusd_resource): (ComponentAddress, ResourceAddress) = 
        scrypto_decode(&out).unwrap();

      let out = ScryptoVmV1Api::blueprint_call(
            eux_p,
            "Eux",
            "from_nothing",
            scrypto_args!(
              alpha_addr, power_azero,
              power_eux, eusd_resource, exrd, e_swap_fee, bang
            )
        );
      let (eux_addr, euxlp_resource): (ComponentAddress, ResourceAddress) = 
        scrypto_decode(&out).unwrap();
      
      the_zero.as_fungible().authorize_with_all(|| {
        let alpha: Global<AnyComponent> = alpha_addr.into();
        alpha.call_raw::<()>(
          "set_app_addrs", scrypto_args!((usd_addr, eux_addr, bang))
        );
        let dao: Global<AnyComponent> = dao_addr.into();
        dao.call_raw::<()>(
          "set_phase2_args", scrypto_args!(
            euxlp_resource
          )
        );
        dao.call_raw::<()>(
          "set_branch_addrs", scrypto_args!((alpha_addr, bang, bang))
        )
      });

      the_zero.burn();
      
      // todo remove azero
      (dao_addr, oracle1.into(), oracle2.into())
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
              alpha_addr, self.power_azero,
              self.power_tri.take_all(),
              self.real.resource_address(), t_w1,
              self.euxlp, t_w2,
              t_swap_fee,
              self.bang
            )
        );
      let tri_addr: ComponentAddress = 
        scrypto_decode(&out).unwrap();

      let delta_resource = self.power_delta.resource_address();

      let amnt = self.real.amount();
      let aa_real = self.real.take(dec!("0.003")*amnt);

      let out = ScryptoVmV1Api::blueprint_call(
            self.delta_p,
            "Delta",
            "from_nothing",
            scrypto_args!(
              dao_addr, self.power_zero,
              alpha_resource, self.power_delta.take_all(),
              aa_real, 
              self.euxlp,
              self.bang
            )
        );
      let delta_addr: ComponentAddress = 
        scrypto_decode(&out).unwrap();

        let out = ScryptoVmV1Api::blueprint_call(
            self.omega_p,
            "Omega",
            "from_nothing",
            scrypto_args!(
              dao_addr, self.power_zero,
              delta_resource, self.power_omega.take_all(),
              self.real.take_all(),
              self.bang
            )
        );
      let omega_addr: ComponentAddress = 
        scrypto_decode(&out).unwrap();

      self.set_branch_addrs((alpha_addr, delta_addr, omega_addr));

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

      // the dao is dead, long live the dao
      ScryptoVmV1Api::blueprint_call(
        self.daov2_p,
        "Dao",
        "from_something",
        scrypto_args!(
          self.power_dao.take_all(),
          self.power_zero,
          self.souls,
          self.branch_addrs,
          self.bang
        )
      );
    }

    pub fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress) {
      self.branch_addrs
    }

    pub fn set_branch_addrs(&mut self, new: (ComponentAddress, ComponentAddress, ComponentAddress)) {
      self.branch_addrs = new;
    }

    // phase-braiding functions

    pub fn set_phase2_args(&mut self, euxlp: ResourceAddress) {
      self.euxlp = euxlp;
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
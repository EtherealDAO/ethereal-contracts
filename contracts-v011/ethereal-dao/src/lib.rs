use scrypto::prelude::*;

#[blueprint]
mod dao {

  // static-participant multisig 
  // self-governed via 3/3, each participant being a DAO branch
  struct Dao {
    power_dao: Vault,
    souls: (ResourceAddress, ResourceAddress, ResourceAddress),
    power_zero: ResourceAddress,

    // alpha, Delta, omega
    branch_addrs: (ComponentAddress, ComponentAddress, ComponentAddress)
  }

  impl Dao {
    pub fn from_nothing( // todo Omega
      alpha_p: PackageAddress, delta_p: PackageAddress,
      usd_p: PackageAddress, eux_p: PackageAddress, tri_p: PackageAddress,
      real: Bucket, exrd: ResourceAddress
      ) -> Global<Dao> {
      // todo for now just a mock script helping the setup/reproducible redeploy
      // + addr beacon

      let U_LOWER = dec!("0.99");
      let U_UPPER = dec!("1.01");
      let U_FLASH_FEE = dec!("1.001");
      let U_MOCK_ORACLE = dec!("1");

      let E_SWAP_FEE = dec!("0.997");
      
      let T_W1 = dec!("0.90");
      let T_W2 = dec!("0.10");
      let T_SWAP_FEE = dec!("0.997");

      let power_dao = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);
      let power_alpha = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);
      let power_delta = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);
      let power_omega = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(1);

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

      let bang = 
        ComponentAddress::virtual_identity_from_public_key(
          &PublicKey::EcdsaSecp256k1(
            EcdsaSecp256k1PublicKey::from_str(
              "0345495dce6516c31862d36d1d0b254fad29ab016b6d972ebac1dd3902a41b0f9b").unwrap()
          )
        );

      let the__zero = power_dao.as_fungible().authorize_with_all(|| 
        ResourceManager::from(power_zero).mint(1)
      );

      let dao_addr = Self {
        power_dao: Vault::with_bucket(power_dao),
        souls: (power_alpha, power_delta, power_omega),
        power_zero.clone(),

        branch_addrs: (bang, bang, bang)
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .globalize()
      .address();

      let alpha_resource = power_alpha.resource_address();
      let delta_resource = power_delta.resource_address();
      let omega_resource = power_omega.resource_address();


      let alpha_addr = alpha_p.call_raw::<ComponentAddress>(
        "from_nothing", scrypto_args!(
          dao_addr, power_zero,
          power_alpha, power_azero,
          bang, bang, bang
        )
      );

      let (usd_addr, eusd_resource) = usd_p.call_raw::<(ComponentAddress, ResourceAddress)>(
        "from_nothing", scrypto_args!(
          dao_addr, alpha_resource,
          power_eux.resource_address().clone(), power_usd,
          exrd, U_LOWER, U_UPPER, U_FLASH_FEE, U_MOCK_ORACLE
        )
      );

      let (eux_addr, euxlp_resource) = eux_p.call_raw::<(ComponentAddress, ResourceAddress)>(
        "from_nothing", scrypto_args!(
          alpha_addr, alpha_resource, power_azero,
          power_eux, eusd_resource, exrd, E_SWAP_FEE
        )
      );

      let (tri_addr, tlp_resource) = eux_p.call_raw::<(ComponentAddress, ResourceAddress)>(
        "from_nothing", scrypto_args!(
          alpha_addr, alpha_resource, power_azero,
          power_tri,
          real.resource_address(), T_W1,
          exrd, T_W2,
          T_SWAP_FEE
        )
      );

      the__zero.as_fungible().authorize_with_all(|| 
        alpha_addr.call_raw::<()>(
          "set_app_addrs", scrypto_args!((usd_addr, eux_addr, tri_addr))
        )
      );

      let delta_addr = alpha_p.call_raw::<ComponentAddress>(
        "from_nothing", scrypto_args!(
          dao_addr, power_zero,
          alpha_resource, power_delta,
          vec![
            (XRD, dec!(0)), (real.resource_address(), dec!(0)), 
            (eusd_resource, dec!(0)), (euxlp_resource, dec!(0))
            (exrd, dec!(0)), (tlp_resource, dec!(0))],
          real, // TODO for now drops ALL real into AA use
          euxlp_resource,
          tlp_resource
        )
      );

      // TODO omega

      the__zero.as_fungible().authorize_with_all(|| 
        dao_addr.call_raw::<()>(
          "set_branch_addrs", scrypto_args!((alpha_addr, delta_addr, bang)) // TOOD
        )
      );

      the__zero.burn();
    }

    pub fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress) {
      self.branch_addrs
    }

    pub fn set_branch_addrs(&mut self, ) {
      self.branch_addrs = new;
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
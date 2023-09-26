use scrypto::prelude::*;

#[blueprint]
mod upusd {
  struct UpUsd {
    power_azero: Vault
  }

  impl UpUsd {
    // split into setup and execute 
    // in order to make update params not pass the addrs
    pub fn setup(power_azero: Bucket) -> ComponentAddress {
      Self {
        power_azero: Vault::with_bucket(power_azero)
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .globalize()
      .address()
    }

    // potentially needs better variable naming lmao
    pub fn execute(&mut self, 
      usd_addr: ComponentAddress, alpha_addr: ComponentAddress, new_usd_addr: PackageAddress,
      dapp_def_addr: ComponentAddress, p_eux_addr: ResourceAddress, valid_addr: ComponentAddress,
      oracle1_addr: ResourceAddress, oracle2_addr: ResourceAddress, ecdp_addr: ResourceAddress,
      eusd_addr: ResourceAddress) {
      // lets of manual setup but it's useful for static verification
      let usd_static = "component_tdx_2_1czptk7mmszu79xq6ed70en4aecpg4yyn5j3dyc2z338qcdplu6zxdt";
      let alpha_static = "component_tdx_2_1crk5paafj5mzqselkvkj0440veh3ys4f0erntnvdx8zva94cqc9wdr";
      let new_usd_static = "package_tdx_2_1p5slrtkre5075rv3444tgx4lnursfg0cdwkfjedxe375zszhh6tnns";
      let dapp_def_static = "account_tdx_2_128rnhjdevjd8yqqdzpgc4sd39k8cspu9gakkw2p4h5yj65zhp072cd";
      // TODO add souls query on alpha?? (next update test)
      // for now can just dig out of the daov1 init tx
      let p_eux_static = "resource_tdx_2_1t5xchytmydrttkfjaxfuu43e0ukj8hcp26d5g6vpfmuynkurdcxjux";
      let valid_static = "validator_tdx_2_1svh0gk33hg7rgh0kgy9ze78vq0h7qrsgtxn8y72s5g9mmq08rq6ljj";
      let oracle1_static = "resource_tdx_2_1t4wv8kt052uc2xngakrre5l05rm78up2kh5arhrrfla509mepyt0ft";
      let oracle2_static = "resource_tdx_2_1t5u7pzzz9aqr7c63fnt6rc8ml6zwn97wpnzrylu664edl0vekgzt5w";
      let ecdp_static = "resource_tdx_2_1ngztdjth9famgej5n9r0knnfcec6mfp7n8yyhh8ctp0jrknmlr0y4c";
      let eusd_static = "resource_tdx_2_1t4xfmc5s9jp3a2uaekq6y6dl7sg204lutc245v5v52s682g3eqwx29";

      let decoder = AddressBech32Decoder::new(
        &NetworkDefinition { 
          id: 0x2,
          logical_name: "stokenet".to_owned(),
          hrp_suffix: "tdx_2_".to_owned() });

      // this seems silly but it's just about briging the addrs into scope
      // only do for ones that are *called*, not just passed as args
      assert!( 
        usd_addr == ComponentAddress::try_from_bech32(&decoder, usd_static).unwrap() && 
        alpha_addr == ComponentAddress::try_from_bech32(&decoder, alpha_static).unwrap() &&
        new_usd_addr == PackageAddress::try_from_bech32(&decoder, new_usd_static).unwrap() &&
        dapp_def_addr == ComponentAddress::try_from_bech32(&decoder, dapp_def_static).unwrap() &&
        p_eux_addr == ResourceAddress::try_from_bech32(&decoder, p_eux_static).unwrap() &&
        valid_addr == ComponentAddress::try_from_bech32(&decoder, valid_static).unwrap() &&
        oracle1_addr == ResourceAddress::try_from_bech32(&decoder, oracle1_static).unwrap() &&
        oracle2_addr == ResourceAddress::try_from_bech32(&decoder, oracle2_static).unwrap() &&
        ecdp_addr == ResourceAddress::try_from_bech32(&decoder, ecdp_static).unwrap() &&
        eusd_addr == ResourceAddress::try_from_bech32(&decoder, eusd_static).unwrap(),
        "wrong addrs");

      let a0 = self.power_azero.take_all();
      a0.as_fungible().authorize_with_amount(dec!(1), || {
        let usd: Global<AnyComponent> = usd_addr.into();
        let alpha: Global<AnyComponent> = alpha_addr.into();

        let (pu, exrd, xrd) = 
          usd.call_raw::<(Bucket, Bucket, Bucket)>(
            "to_nothing", scrypto_args!()
          );

        let (alpt, llpt, lt, oracle) = 
          usd.call_raw::<(Decimal, Decimal, Decimal, Decimal)>(
            "look_within", scrypto_args!()
          );

        let (ep, mcr, bp, lower_bound, upper_bound, mm, ff) = 
          usd.call_raw::<(Decimal, Decimal, Decimal, Decimal, Decimal, Decimal, Decimal)>(
            "get_params", scrypto_args!()
          );

        let out = ScryptoVmV1Api::blueprint_call(
          new_usd_addr,
          "Usd",
          "from_something",
          scrypto_args!(
            alpha_addr, self.power_azero.resource_address(),
            p_eux_addr, pu,
            valid_addr,
            lower_bound, upper_bound, ff, 
            dapp_def_addr,
            oracle, oracle1_addr, oracle2_addr,
            exrd, xrd, ecdp_addr,
            eusd_addr,
            alpt, llpt, lt,
            ep, mcr, bp, mm
          )
        );
        let new_usd_addr: ComponentAddress = 
          scrypto_decode(&out).unwrap();

        let (_, e, t) = alpha.call_raw::<(ComponentAddress, ComponentAddress, ComponentAddress)>(
          "get_app_addrs", scrypto_args!()
        );
        alpha.call_raw::<()>(
          "set_app_addrs", scrypto_args!((new_usd_addr, e, t))
        );
      });

      a0.burn();
    }

  }
}
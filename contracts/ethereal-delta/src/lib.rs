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
      add_to_aa => PUBLIC;
      withdraw => restrict_to: [alpha];
      prove_delta => restrict_to: [alpha];
      set_dao_addr => restrict_to: [zero];
    }
  }

  
  struct Delta {
    dao_addr: ComponentAddress,
    power_delta: Vault,

    // (REAL, EUXLP), that will be used for AA
    // REAL (to be paired with EUXLP)
    // EUXLP (stored, if ever REAL is depleted)
    aa_treasury: (Vault, Vault),
    treasury: KeyValueStore<ResourceAddress, Vault>,
  }

  impl Delta {
    pub fn from_nothing(dao_addr: ComponentAddress, power_zero: ResourceAddress,
      power_alpha: ResourceAddress, power_delta: Bucket,
      real:Bucket, euxlp: ResourceAddress, bang: ComponentAddress
    ) -> ComponentAddress {
      // needs to whitelist
      // real, tlp, euxlp, exrd, xrd, eusd

      let aa_treasury = (Vault::with_bucket(real), Vault::new(euxlp));

      let treasury = KeyValueStore::new();

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
            "tags" => vec!["ethereal-dao".to_owned(), 
              "delta".to_owned()], updatable;
          }
        )
      )
      .globalize()
      .address()
    }

    pub fn to_nothing(&mut self) -> Bucket {
      self.power_delta.take_all()
    }

    pub fn deposit(&mut self, input: Bucket) {
      if let Some(mut v) = self.treasury.get_mut(&input.resource_address()) {
        v.deref_mut().put(input);
        return
      };
      self.treasury.insert(input.resource_address(), Vault::with_bucket(input))
    }

    pub fn withdraw(&mut self, resource: ResourceAddress, amount: Decimal) -> Bucket {
      if let Some(mut v) = self.treasury.get_mut(&resource) {
        return v.deref_mut().take(amount)
      } 
      panic!("no resource found");
    }

    // add REAL to AA real -- future update could allow more complex manipulation 
    // back and forth
    pub fn add_to_aa(&mut self, input: Bucket) {
      self.aa_treasury.0.put(input)
    }

    // GIVE ME ALL OF THE REAL
    // and, if you've any EUXLP, that also
    pub fn aa_tap(&mut self) -> (Option<Bucket>, Option<Bucket>) {
      info!("aa_top IN"); 

      // honestly it pulling all at once is a hack to add miaximum possible size
      // without doing any calculation
      ( 
        if self.aa_treasury.0.is_empty() { None } else { Some(self.aa_treasury.0.take_all()) }, 
        if self.aa_treasury.1.is_empty() { None } else { Some(self.aa_treasury.1.take_all()) }
      )
    }

    // thank you for being real with me
    pub fn aa_out(&mut self, ret: Option<Bucket>) {
      info!("aa_out IN"); 

      if let Some(r) = ret {
        if r.resource_address() == self.aa_treasury.0.resource_address() {
          self.aa_treasury.0.put(r);
        } else {
          self.aa_treasury.1.put(r);
        }
      }
    }

    // pupeteer delta by alpha
    pub fn prove_delta(&self) -> FungibleProof {
      self.power_delta.as_fungible().create_proof_of_amount(dec!(1))
    }

    pub fn set_dao_addr(&mut self, new: ComponentAddress) {
      self.dao_addr = new;
    }

    // todo move from treasury to aa treasury
  }
}
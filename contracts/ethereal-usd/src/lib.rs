use scrypto::prelude::*;

#[derive(ScryptoSbor, NonFungibleData)]
pub struct UserReceipt {
  #[mutable]
  // counts the XRD in protocol (the debt will be mandatorily pegged)
  // gets updated remotely (can it? would be nice if it could)
  // should work: TODO: store the UUID id in the CDP contract
  protocol_lp: Decimal
  #[mutable]
  // REAL (or REAL/XRD LP), protocol LP, eUSD/XRD LP
  staked_token_amount: (Decimal, Decimal, Decimal), 
  #[mutable]
  // amount "claimed" per rewards vault
  rewards_claimed: Vec<Decimal>, 
  top_voted_index: u64 // can't vote for lower than this 
}

#[blueprint]
mod ethereal {
  struct Ethereal {
    assets: Vault,
    assets_resource: ResourceAddress,

    liabilities: Decimal,
    liability_badge: Vault,
    liability_resource: ResourceAddress,

    // TODO add oracle (for now 1:1)

    // list of pairs of assets to liabilities
    cdps: Vec<(Decimal, Decimal)>, // TODO store user UUID

    owner_resource: ResourceAddress,
    owner_badge: Vault
  }

  impl Ethereal {
    pub fn instantiate_ethereal(init_a: Bucket) 
      -> ComponentAddress {

      let liability_badge = Vault::with_bucket(
        ResourceBuilder::new_fungible()
          .divisibility(DIVISIBILITY_NONE)
          .mint_initial_supply(1)
        );

      let liability_resource = 
        ResourceBuilder::new_fungible()
        .mintable(rule!(require(liability_badge.resource_address())), LOCKED)
        .burnable(rule!(require(liability_badge.resource_address())), LOCKED)
        .metadata("name", "SOMETHING1")
        // doesn't work currently, wallet doesn't implement icons yet
        .metadata("icon", "https://ethereal.systems/logos2.jpeg")
        .create_with_no_initial_supply();
      
      let owner_badge = Vault::with_bucket(
        ResourceBuilder::new_fungible()
          .divisibility(DIVISIBILITY_NONE)
          .mint_initial_supply(1)
        );

      let owner_resource = 
        ResourceBuilder::new_uuid_non_fungible::<()>()
          .mintable(rule!(require(owner_badge.resource_address())), LOCKED)
          .burnable(rule!(require(owner_badge.resource_address())), LOCKED)
          // recall for cleaning up post liquidation
          .recallable(rule!(require(owner_badge.resource_address())), LOCKED)
          .metadata("name", "SOMETHING1")
          .create_with_no_initial_supply();

      let assets_resource = init_a.resource_address();
      let a = init_a.amount();

      Self {
        assets: Vault::with_bucket(init_a),
        assets_resource: assets_resource,

        liabilities: dec!(0),
        liability_badge: liability_badge,
        liability_resource: liability_resource,

        cdps: vec![(a, dec!(0))],

        owner_badge: owner_badge,
        owner_resource: owner_resource
      }
      .instantiate()
      .globalize()
    }

    // create cdp, deposit moolah
    // does not permit minting 
    // up to frontend to call deposit instead of this
    pub fn create_cdp(&mut self, input: Bucket) -> Bucket {
      // TODO assert minimum deposit amount
      
      let a = input.amount();
      self.cdps.push((a,dec!(0)));
      self.assets.put(input);

      return self.owner_badge.authorize(|| 
        borrow_resource_manager!(self.owner_resource)
        .mint_uuid_non_fungible(()))
    }

  }

}
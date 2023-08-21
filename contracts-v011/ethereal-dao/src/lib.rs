use scrypto::prelude::*;

// let component: Global<AnyComponent> = Global(ObjectStub::new(ObjectStubHandle::Global(GlobalAddress::from(component_address))));
// let return_value = component.call_raw::<ZygomebFancyReturnType>("method_name", scrypto_args!(param1));

#[blueprint]
mod dao {

  // static-participant multisig 
  // self-governed via 3/3, each participant being a DAO branch
  struct Dao {
    alpha_addr: ComponentAddress
  }

  impl Dao {
    pub fn from_nothing(alpha_addr: ComponentAddress) -> Global<Dao> {
      Self {
        alpha_addr: alpha_addr,
      }
      .instantiate()
      .prepare_to_globalize(OwnerRole::None)
      .globalize()
    }

    pub fn get_branch_addrs(&self) -> (ComponentAddress, ComponentAddress, ComponentAddress) {
      (self.alpha_addr, self.alpha_addr, self.alpha_addr)
    }
  }
}
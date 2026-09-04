use runquiry_core::{
    CapabilityStatus, InspectError, ProcessAction, ProcessController, ProcessIdentity,
};

use super::FakePlatform;

impl ProcessController for FakePlatform {
    fn capability(&self) -> CapabilityStatus {
        self.capability_for()
    }

    fn execute(
        &self,
        identity: &ProcessIdentity,
        action: ProcessAction,
    ) -> Result<(), InspectError> {
        if !identity.same_process(&self.current) {
            return Err(InspectError::ProcessChanged {
                identity: self.current.clone(),
            });
        }
        self.executed
            .borrow_mut()
            .push((self.current.pid(), action));
        Ok(())
    }
}

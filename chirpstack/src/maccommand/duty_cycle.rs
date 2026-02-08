use anyhow::Result;

use crate::storage::device;

pub fn request(max_duty_cycle: u8) -> lrwn::MACCommandSet {
    lrwn::MACCommandSet::new(vec![lrwn::MACCommand::DutyCycleReq(
        lrwn::DutyCycleReqPayload { max_duty_cycle },
    )])
}

pub fn handle(
    dev: &mut device::Device,
    block: &lrwn::MACCommandSet,
    pending: Option<&lrwn::MACCommandSet>,
) -> Result<Option<lrwn::MACCommandSet>> {
    let ds = dev.get_device_session_mut()?;

    if pending.is_none() {
        return Err(anyhow!("Pending DutyCycleReq expected"));
    }

    let block_macs = &**block;
    let pending_macs = &**pending.unwrap();

    let lrwn::MACCommand::DutyCycleReq(pl) = pending_macs
        .first()
        .ok_or_else(|| anyhow!("Empty MACCommandSet"))?
    else {
        return Err(anyhow!("Expected DutyCycleReq"));
    };

    let lrwn::MACCommand::DutyCycleAns = block_macs
        .first()
        .ok_or_else(|| anyhow!("Empty MACCommandSet"))?
    else {
        return Err(anyhow!("Expected DutyCycleAns"));
    };

    ds.max_duty_cycle = pl.max_duty_cycle as u32;

    Ok(None)
}

#[cfg(test)]
pub mod test {
    use super::*;
    use chirpstack_api::internal;

    struct Test {
        name: String,
        device_session: internal::DeviceSession,
        duty_cycle_req: Option<lrwn::MACCommandSet>,
        duty_cycle_ans: lrwn::MACCommandSet,
        expected_device_session: internal::DeviceSession,
        expected_error: Option<String>,
    }

    #[test]
    fn test_request() {
        let resp = request(9);
        assert_eq!(
            lrwn::MACCommandSet::new(vec![lrwn::MACCommand::DutyCycleReq(
                lrwn::DutyCycleReqPayload { max_duty_cycle: 9 }
            )]),
            resp
        );
    }

    #[test]
    fn test_handle() {
        let tests = vec![
            Test {
                name: "pending request and positive ACK updates max_duty_cycle".into(),
                device_session: internal::DeviceSession {
                    max_duty_cycle: 0,
                    ..Default::default()
                },
                duty_cycle_req: Some(lrwn::MACCommandSet::new(vec![
                    lrwn::MACCommand::DutyCycleReq(lrwn::DutyCycleReqPayload {
                        max_duty_cycle: 14,
                    }),
                ])),
                duty_cycle_ans: lrwn::MACCommandSet::new(vec![lrwn::MACCommand::DutyCycleAns]),
                expected_device_session: internal::DeviceSession {
                    max_duty_cycle: 14,
                    ..Default::default()
                },
                expected_error: None,
            },
            Test {
                name: "no pending request and positive ACK returns an error".into(),
                device_session: internal::DeviceSession {
                    max_duty_cycle: 5,
                    ..Default::default()
                },
                duty_cycle_req: None,
                duty_cycle_ans: lrwn::MACCommandSet::new(vec![lrwn::MACCommand::DutyCycleAns]),
                expected_device_session: internal::DeviceSession {
                    max_duty_cycle: 5,
                    ..Default::default()
                },
                expected_error: Some("Pending DutyCycleReq expected".to_string()),
            },
        ];

        for tst in &tests {
            let mut dev = device::Device {
                device_session: Some(tst.device_session.clone().into()),
                ..Default::default()
            };
            let resp = handle(&mut dev, &tst.duty_cycle_ans, tst.duty_cycle_req.as_ref());

            if let Some(e) = &tst.expected_error {
                assert!(resp.is_err(), "{}", tst.name);
                assert_eq!(e, &format!("{}", resp.err().unwrap()), "{}", tst.name);
            } else {
                assert!(resp.unwrap().is_none());
            }

            assert_eq!(
                &tst.expected_device_session,
                dev.get_device_session().unwrap()
            );
        }
    }
}

import grpc, time
from chirpstack_api import api

# Configuration.

# This must point to the API interface.
server = "localhost:8080"

# The API token (retrieved using the web-interface).
api_token = "..."

# The tenant id (retrieved using the web-interface)
tenant_id = "..."

if __name__ == "__main__":
    # Connect without using TLS.
    with grpc.insecure_channel(server) as channel:
        # Define the API key meta-data.
        auth_token = [("authorization", "Bearer %s" % api_token)]

        # Retrieve all application ids
        client = api.ApplicationServiceStub(channel)
        resp: api.ListApplicationsResponse = client.List(
            api.ListApplicationsRequest(
                limit=100,
                tenant_id=tenant_id,
            ),
            metadata=auth_token,
        )
        app_ids = [app.id for app in resp.result]
        print(f"Application ids: {app_ids}")

        # Retrieve all dev_euis
        client = api.DeviceServiceStub(channel)
        dev_euis = [
            dev.dev_eui
            for application_id in app_ids
            for dev in client.List(
                api.ListDevicesRequest(
                    limit=100,
                    application_id=application_id,
                ),
                metadata=auth_token,
            ).result
        ]
        print(f"Dev EUIs: {dev_euis}\n")

        # First device
        dev_eui = dev_euis[0]

        # Desired uplink channels
        chmask = [0, 5, 7]

        # Device config store API client.
        client = api.DeviceConfigStoreServiceStub(channel)

        # Check available uplink channels
        resp: api.GetDeviceCurrentParamsResponse = client.GetDeviceCurrentParams(
            api.GetDeviceCurrentParamsRequest(dev_eui=dev_eui),
            metadata=auth_token,
        )
        uplink_channels = resp.channels
        # Verify configuration feasibility
        while any((ch_id not in uplink_channels for ch_id in chmask)):
            time.sleep(5)
            resp = client.GetDeviceCurrentParams(
                api.GetDeviceCurrentParamsRequest(dev_eui=dev_eui),
                metadata=auth_token,
            )
            uplink_channels = resp.channels
        print(f"Available uplink channels: {uplink_channels}\n")

        # Set chmask config
        resp = client.Set(
            api.SetDeviceConfigStoreRequest(
                dev_eui=dev_eui,
                device_config_store=api.DeviceConfigStore(
                    enabled_uplink_channel_indices=chmask,
                    dr=5,
                    tx_power_index=3,
                    nb_trans=2,
                    max_duty_cycle=7,
                ),
            ),
            metadata=auth_token,
        )
        print(f"Set response: {resp}\n")

        resp = client.Get(
            api.GetDeviceConfigStoreRequest(dev_eui=dev_eui),
            metadata=auth_token,
        )
        print(f"Get response: {resp}\n")

        aligned = False
        while not aligned:
            time.sleep(5)
            resp: api.GetDeviceConfigAlignmentResponse = (
                client.GetDeviceConfigAlignment(
                    api.GetDeviceConfigAlignmentRequest(dev_eui=dev_eui),
                    metadata=auth_token,
                )
            )
            print(f"Alignment status: {resp}")
            aligned = (
                resp.enabled_uplink_channel_indices
                and resp.dr
                and resp.tx_power_index
                and resp.nb_trans
                and resp.max_duty_cycle
            )

        input("\nPress enter to clean-up configs and terminate program...")

        resp = client.Delete(
            api.DeleteDeviceConfigStoreRequest(dev_eui=dev_eui),
            metadata=auth_token,
        )
        print(f"\nDelete response: {resp}")

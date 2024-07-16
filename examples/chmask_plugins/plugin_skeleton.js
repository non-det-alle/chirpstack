// This must return the name of the CHMASK algorithm.
export function name() {
  return "Example plugin";
}

// This must return the id of the CHMASK algorithm.
export function id() {
  return "example_id";
}

// This handles the CHMASK request.
//
// Input object example:
// {
//  regionConfigId: "eu868",
//  regionCommonName: "EU868",
//  devEui: "0102030405060708",
//  macVersion: "1.0.3",
//  regParamsRevision: "A",
//  uplinkChannels: [
//    {}
//  ],
//  uplinkHistory: [
//    {
//      "fCnt": 10,
//      "maxSnr": 7.5,
//      "maxRssi": -110,
//      "txPowerIndex": 0,
//      "gatewayCount": 3
//    }
//  ]
// }
//
// This function must return a list of indices, example:
// [1, 3]
// 
export function handle(req) {
  return [1, 3];
}

{
    "name": "MotorControl",
    "rungs": [
        {
            "comment": "Seal-in motor: (Start OR Motor) AND NOT Stop -> MotorRun",
            "branches": [
                {
                    "elements": [{ "name": "Start", "negated": false }]
                },
                {
                    "elements": [{ "name": "Motor", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "coil",
                    "name": "MotorRun",
                    "variant": "normal"
                }
            ]
        },
        {
            "comment": "Timer: MotorRun AND NOT Done -> TON(PT=T#5s) -> Done",
            "branches": [
                {
                    "elements": [
                        { "name": "MotorRun", "negated": false },
                        { "name": "Done", "negated": true }
                    ]
                }
            ],
            "outputs": [
                {
                    "kind": "block",
                    "fb_type": "TON",
                    "instance": "Timer",
                    "inputs": [
                        { "name": "IN", "value": "MotorRun" },
                        { "name": "PT", "value": "T#5s" }
                    ],
                    "outputs": [
                        { "name": "Q", "value": "Done" }
                    ]
                }
            ]
        },
        {
            "comment": "Counter: Pulse -> CTU(PV=10) -> Reached",
            "branches": [
                {
                    "elements": [{ "name": "Pulse", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "block",
                    "fb_type": "CTU",
                    "instance": "Counter",
                    "inputs": [
                        { "name": "CU", "value": "Pulse" },
                        { "name": "PV", "value": "10" }
                    ],
                    "outputs": [
                        { "name": "Q", "value": "Reached" }
                    ]
                }
            ]
        },
        {
            "comment": "Reset motor when timer done",
            "branches": [
                {
                    "elements": [{ "name": "Done", "negated": false }]
                }
            ],
            "outputs": [
                {
                    "kind": "coil",
                    "name": "MotorRun",
                    "variant": "reset"
                }
            ]
        }
    ]
}

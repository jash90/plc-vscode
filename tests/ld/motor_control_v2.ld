{
  "name": "MotorControl",
  "schema_version": 2,
  "rungs": [
    {
      "id": "r0",
      "comment": "Seal-in motor: (Start OR Motor) AND NOT Stop -> MotorRun",
      "branches": [
        {
          "elements": [
            {
              "id": "e0",
              "name": "Start",
              "negated": false
            }
          ]
        },
        {
          "elements": [
            {
              "id": "e1",
              "name": "Motor",
              "negated": false
            }
          ]
        }
      ],
      "outputs": [
        {
          "kind": "coil",
          "id": "e2",
          "name": "MotorRun",
          "variant": "normal"
        }
      ]
    },
    {
      "id": "r1",
      "comment": "Timer: MotorRun AND NOT Done -> TON(PT=T#5s) -> Done",
      "branches": [
        {
          "elements": [
            {
              "id": "e3",
              "name": "MotorRun",
              "negated": false
            },
            {
              "id": "e4",
              "name": "Done",
              "negated": true
            }
          ]
        }
      ],
      "outputs": [
        {
          "kind": "block",
          "id": "e5",
          "fb_type": "TON",
          "instance": "Timer",
          "inputs": [
            {
              "name": "IN",
              "value": "MotorRun"
            },
            {
              "name": "PT",
              "value": "T#5s"
            }
          ],
          "outputs": [
            {
              "name": "Q",
              "value": "Done"
            }
          ]
        }
      ]
    },
    {
      "id": "r2",
      "comment": "Counter: Pulse -> CTU(PV=10) -> Reached",
      "branches": [
        {
          "elements": [
            {
              "id": "e6",
              "name": "Pulse",
              "negated": false
            }
          ]
        }
      ],
      "outputs": [
        {
          "kind": "block",
          "id": "e7",
          "fb_type": "CTU",
          "instance": "Counter",
          "inputs": [
            {
              "name": "CU",
              "value": "Pulse"
            },
            {
              "name": "PV",
              "value": "10"
            }
          ],
          "outputs": [
            {
              "name": "Q",
              "value": "Reached"
            }
          ]
        }
      ]
    },
    {
      "id": "r3",
      "comment": "Reset motor when timer done",
      "branches": [
        {
          "elements": [
            {
              "id": "e8",
              "name": "Done",
              "negated": false
            }
          ]
        }
      ],
      "outputs": [
        {
          "kind": "coil",
          "id": "e9",
          "name": "MotorRun",
          "variant": "reset"
        }
      ]
    }
  ]
}

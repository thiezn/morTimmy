const railTrace = "0.9mm"
const heavyRailTrace = "1.2mm"

const BusNodeStrip = ({
    name,
    nodeCount,
    startX,
    startY,
    pitch,
    width,
}: {
    name: string
    nodeCount: number
    startX: number
    startY: number
    pitch: number
    width: string
}) => (
    <>
        {Array.from({ length: nodeCount }, (_, index) => (
            <testpoint
                key={`${name}-node-${index + 1}`}
                name={`${name}_${index + 1}`}
                footprintVariant="through_hole"
                holeDiameter="0.8mm"
                padDiameter="1.6mm"
                pcbX={`${startX}mm`}
                pcbY={`${startY - index * pitch}mm`}
            />
        ))}
        {Array.from({ length: nodeCount - 1 }, (_, index) => (
            <trace
                key={`${name}-bus-${index + 1}`}
                from={`.${name}_${index + 1} > .pin1`}
                to={`.${name}_${index + 2} > .pin1`}
                width={width}
            />
        ))}
    </>
)

const TwoPinPowerConnector = ({
    name,
    railLabel,
    railSource,
    groundSource,
    pcbX,
    pcbY,
    orientation = "horizontal",
    pitch,
    holeDiameter,
    platedDiameter,
    providesPower = false,
    width = railTrace,
}: {
    name: string
    railLabel: string
    railSource: string
    groundSource: string
    pcbX: string
    pcbY: string
    orientation?: "horizontal" | "vertical"
    pitch: string
    holeDiameter: string
    platedDiameter: string
    providesPower?: boolean
    width?: string
}) => (
    <>
        <pinheader
            name={name}
            pinCount={2}
            pitch={pitch}
            holeDiameter={holeDiameter}
            platedDiameter={platedDiameter}
            pcbX={pcbX}
            pcbY={pcbY}
            pcbOrientation={orientation}
            gender="female"
            showSilkscreenPinLabels
            pinLabels={[railLabel, "GND"]}
            pinAttributes={{
                pin1: providesPower
                    ? { providesPower: true, requiresPower: true }
                    : { requiresPower: true },
                pin2: { requiresGround: true },
            }}
        />
        <trace from={`.${name} > .pin1`} to={railSource} width={width} />
        <trace from={`.${name} > .pin2`} to={groundSource} width={width} />
    </>
)

export default () => (
    <board width="140mm" height="90mm" layers={2}>
        <chip
            name="J_BAT"
            footprint="kicad:Connector_AMASS/AMASS_XT30UPB-M_1x02_P5.0mm_Vertical"
            pcbX="-64mm"
            pcbY="28mm"
            pinLabels={{
                pin1: "VBAT_IN",
                pin2: "GND",
            }}
            pinAttributes={{
                VBAT_IN: { providesPower: true, requiresPower: true },
                GND: { requiresGround: true },
            }}
        />

        <fuse
            name="F1"
            currentRating="10A"
            voltageRating="16V"
            footprint="2512"
            pcbX="-48mm"
            pcbY="28mm"
        />

        <diode
            name="D1"
            footprint="smb"
            variant="schottky"
            pcbX="-34mm"
            pcbY="28mm"
        />

        <capacitor
            name="C_BAT_BULK"
            capacitance="47uF"
            footprint="1210"
            pcbX="-20mm"
            pcbY="36mm"
        />
        <capacitor
            name="C_BAT_HF"
            capacitance="100nF"
            footprint="0603"
            pcbX="-20mm"
            pcbY="20mm"
        />

        <BusNodeStrip
            name="BUS_VBAT"
            nodeCount={8}
            startX={-22}
            startY={28}
            pitch={7}
            width={heavyRailTrace}
        />
        <BusNodeStrip
            name="BUS_GND_MAIN"
            nodeCount={8}
            startX={-10}
            startY={28}
            pitch={7}
            width={heavyRailTrace}
        />
        <BusNodeStrip
            name="BUS_5V"
            nodeCount={9}
            startX={24}
            startY={28}
            pitch={7}
            width={railTrace}
        />
        <BusNodeStrip
            name="BUS_GND_RIGHT"
            nodeCount={10}
            startX={36}
            startY={28}
            pitch={7}
            width={railTrace}
        />
        <BusNodeStrip
            name="BUS_6V"
            nodeCount={4}
            startX={0}
            startY={-7}
            pitch={7}
            width={railTrace}
        />
        <BusNodeStrip
            name="BUS_GND_6V"
            nodeCount={4}
            startX={12}
            startY={-7}
            pitch={7}
            width={railTrace}
        />
        <BusNodeStrip
            name="BUS_3V3"
            nodeCount={4}
            startX={44}
            startY={-10}
            pitch={7}
            width={railTrace}
        />

        <trace from=".J_BAT > .pin1" to=".F1 > .pin1" width={heavyRailTrace} />
        <trace from=".F1 > .pin2" to=".D1 > .anode" width={heavyRailTrace} />
        <trace from=".D1 > .cathode" to=".BUS_VBAT_1 > .pin1" width={heavyRailTrace} />
        <trace from=".J_BAT > .pin2" to=".BUS_GND_MAIN_1 > .pin1" width={heavyRailTrace} />
        <trace from=".C_BAT_BULK > .pin1" to=".BUS_VBAT_2 > .pin1" width={heavyRailTrace} />
        <trace from=".C_BAT_BULK > .pin2" to=".BUS_GND_MAIN_2 > .pin1" width={heavyRailTrace} />
        <trace from=".C_BAT_HF > .pin1" to=".BUS_VBAT_3 > .pin1" width={railTrace} />
        <trace from=".C_BAT_HF > .pin2" to=".BUS_GND_MAIN_3 > .pin1" width={railTrace} />

        <TwoPinPowerConnector
            name="J_5V_UBEC_IN"
            railLabel="VIN"
            railSource=".BUS_VBAT_4 > .pin1"
            groundSource=".BUS_GND_MAIN_4 > .pin1"
            pcbX="-46mm"
            pcbY="12mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            width={heavyRailTrace}
        />
        <TwoPinPowerConnector
            name="J_5V_UBEC_OUT"
            railLabel="5V"
            railSource=".BUS_5V_1 > .pin1"
            groundSource=".BUS_GND_RIGHT_1 > .pin1"
            pcbX="6mm"
            pcbY="28mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            providesPower
            width={heavyRailTrace}
        />

        <TwoPinPowerConnector
            name="J_6V_UBEC_IN"
            railLabel="VIN"
            railSource=".BUS_VBAT_5 > .pin1"
            groundSource=".BUS_GND_MAIN_5 > .pin1"
            pcbX="-46mm"
            pcbY="2mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            width={heavyRailTrace}
        />
        <TwoPinPowerConnector
            name="J_6V_UBEC_OUT"
            railLabel="6V"
            railSource=".BUS_6V_1 > .pin1"
            groundSource=".BUS_GND_6V_1 > .pin1"
            pcbX="18mm"
            pcbY="-7mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            providesPower
            width={railTrace}
        />

        <TwoPinPowerConnector
            name="J_3V3_REG_IN"
            railLabel="5V_IN"
            railSource=".BUS_5V_8 > .pin1"
            groundSource=".BUS_GND_RIGHT_8 > .pin1"
            pcbX="28mm"
            pcbY="-18mm"
            pitch="3.5mm"
            holeDiameter="1mm"
            platedDiameter="2mm"
            width={railTrace}
        />
        <TwoPinPowerConnector
            name="J_3V3_REG_OUT"
            railLabel="3V3"
            railSource=".BUS_3V3_1 > .pin1"
            groundSource=".BUS_GND_RIGHT_7 > .pin1"
            pcbX="28mm"
            pcbY="-10mm"
            pitch="3.5mm"
            holeDiameter="1mm"
            platedDiameter="2mm"
            providesPower
            width={railTrace}
        />

        <trace from=".BUS_GND_MAIN_8 > .pin1" to=".BUS_GND_RIGHT_10 > .pin1" width={railTrace} />
        <trace from=".BUS_GND_MAIN_6 > .pin1" to=".BUS_GND_6V_1 > .pin1" width={railTrace} />
        <trace from=".BUS_GND_MAIN_1 > .pin1" to=".BUS_GND_RIGHT_1 > .pin1" width={railTrace} />

        <capacitor
            name="C_5V_BULK"
            capacitance="47uF"
            footprint="1210"
            pcbX="16mm"
            pcbY="21mm"
        />
        <capacitor
            name="C_6V_BULK"
            capacitance="47uF"
            footprint="1210"
            pcbX="6mm"
            pcbY="-18mm"
        />
        <capacitor
            name="C_3V3_BULK"
            capacitance="22uF"
            footprint="1206"
            pcbX="36mm"
            pcbY="-17mm"
        />

        <trace from=".C_5V_BULK > .pin1" to=".BUS_5V_2 > .pin1" width={railTrace} />
        <trace from=".C_5V_BULK > .pin2" to=".BUS_GND_RIGHT_2 > .pin1" width={railTrace} />
        <trace from=".C_6V_BULK > .pin1" to=".BUS_6V_2 > .pin1" width={railTrace} />
        <trace from=".C_6V_BULK > .pin2" to=".BUS_GND_6V_2 > .pin1" width={railTrace} />
        <trace from=".C_3V3_BULK > .pin1" to=".BUS_3V3_2 > .pin1" width={railTrace} />
        <trace from=".C_3V3_BULK > .pin2" to=".BUS_GND_RIGHT_8 > .pin1" width={railTrace} />

        <TwoPinPowerConnector
            name="J_L298N_A_PWR"
            railLabel="VBAT"
            railSource=".BUS_VBAT_6 > .pin1"
            groundSource=".BUS_GND_MAIN_6 > .pin1"
            pcbX="-50mm"
            pcbY="-10mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            width={heavyRailTrace}
        />
        <TwoPinPowerConnector
            name="J_L298N_B_PWR"
            railLabel="VBAT"
            railSource=".BUS_VBAT_7 > .pin1"
            groundSource=".BUS_GND_MAIN_7 > .pin1"
            pcbX="-50mm"
            pcbY="-22mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            width={heavyRailTrace}
        />

        <TwoPinPowerConnector
            name="J_RPI_PWR"
            railLabel="5V"
            railSource=".BUS_5V_3 > .pin1"
            groundSource=".BUS_GND_RIGHT_3 > .pin1"
            pcbX="56mm"
            pcbY="18mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            width={heavyRailTrace}
        />
        <TwoPinPowerConnector
            name="J_AUDIO_PICO_PWR"
            railLabel="5V"
            railSource=".BUS_5V_4 > .pin1"
            groundSource=".BUS_GND_RIGHT_4 > .pin1"
            pcbX="56mm"
            pcbY="8mm"
            pitch="2.5mm"
            holeDiameter="0.95mm"
            platedDiameter="1.8mm"
        />
        <TwoPinPowerConnector
            name="J_MOTION_PICO_PWR"
            railLabel="5V"
            railSource=".BUS_5V_5 > .pin1"
            groundSource=".BUS_GND_RIGHT_5 > .pin1"
            pcbX="56mm"
            pcbY="-2mm"
            pitch="2.5mm"
            holeDiameter="0.95mm"
            platedDiameter="1.8mm"
        />
        <TwoPinPowerConnector
            name="J_TRELLIS_PWR"
            railLabel="5V"
            railSource=".BUS_5V_6 > .pin1"
            groundSource=".BUS_GND_RIGHT_6 > .pin1"
            pcbX="56mm"
            pcbY="-12mm"
            pitch="2.5mm"
            holeDiameter="0.95mm"
            platedDiameter="1.8mm"
        />
        <TwoPinPowerConnector
            name="J_HCSR04_PWR"
            railLabel="5V"
            railSource=".BUS_5V_7 > .pin1"
            groundSource=".BUS_GND_RIGHT_7 > .pin1"
            pcbX="56mm"
            pcbY="-22mm"
            pitch="2.5mm"
            holeDiameter="0.95mm"
            platedDiameter="1.8mm"
        />
        <TwoPinPowerConnector
            name="J_SERVO_PWR"
            railLabel="6V"
            railSource=".BUS_6V_3 > .pin1"
            groundSource=".BUS_GND_6V_3 > .pin1"
            pcbX="-8mm"
            pcbY="-30mm"
            pitch="5.08mm"
            holeDiameter="1.3mm"
            platedDiameter="2.4mm"
            width={heavyRailTrace}
        />
        <TwoPinPowerConnector
            name="J_SENSOR_3V3_PWR"
            railLabel="3V3"
            railSource=".BUS_3V3_3 > .pin1"
            groundSource=".BUS_GND_RIGHT_9 > .pin1"
            pcbX="56mm"
            pcbY="-32mm"
            pitch="2.5mm"
            holeDiameter="0.95mm"
            platedDiameter="1.8mm"
        />

        <testpoint
            name="TP_VBAT"
            footprintVariant="through_hole"
            holeDiameter="1mm"
            padDiameter="2mm"
            pcbX="-22mm"
            pcbY="-38mm"
        />
        <testpoint
            name="TP_5V"
            footprintVariant="through_hole"
            holeDiameter="1mm"
            padDiameter="2mm"
            pcbX="24mm"
            pcbY="-38mm"
        />
        <testpoint
            name="TP_6V"
            footprintVariant="through_hole"
            holeDiameter="1mm"
            padDiameter="2mm"
            pcbX="0mm"
            pcbY="-38mm"
        />
        <testpoint
            name="TP_3V3"
            footprintVariant="through_hole"
            holeDiameter="1mm"
            padDiameter="2mm"
            pcbX="44mm"
            pcbY="-38mm"
        />
        <testpoint
            name="TP_GND"
            footprintVariant="through_hole"
            holeDiameter="1mm"
            padDiameter="2mm"
            pcbX="36mm"
            pcbY="-38mm"
        />

        <trace from=".TP_VBAT > .pin1" to=".BUS_VBAT_8 > .pin1" width={heavyRailTrace} />
        <trace from=".TP_5V > .pin1" to=".BUS_5V_9 > .pin1" width={railTrace} />
        <trace from=".TP_6V > .pin1" to=".BUS_6V_4 > .pin1" width={railTrace} />
        <trace from=".TP_3V3 > .pin1" to=".BUS_3V3_4 > .pin1" width={railTrace} />
        <trace from=".TP_GND > .pin1" to=".BUS_GND_RIGHT_10 > .pin1" width={railTrace} />
    </board>
)

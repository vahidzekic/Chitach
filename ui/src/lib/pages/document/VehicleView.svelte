<script lang="ts">
  import DocHero from "./DocHero.svelte";
  import DocSections from "./DocSections.svelte";
  import type { VehicleDocument } from "../../api/types";
  import { t } from "../../i18n/index.svelte";
  import { isPast } from "../../util/format";
  import type { BadgeDef, SectionDef } from "./fields";

  let { doc }: { doc: VehicleDocument } = $props();

  // Personal numbers (JMBG) are longer than 9 digits, company numbers (PIB) are not.
  const ownerNoLabel = $derived(
    doc.OwnersPersonalNo.length > 9 ? t("vehicle.ownersPersonalNo") : t("vehicle.ownersCompanyNo"),
  );
  const userNoLabel = $derived(
    doc.UsersPersonalNo.length > 9 || (!doc.UsersPersonalNo && doc.OwnersPersonalNo.length > 9)
      ? t("vehicle.usersPersonalNo")
      : t("vehicle.usersCompanyNo"),
  );
  const hasUser = $derived(!!(doc.UsersSurnameOrBusinessName || doc.UsersName || doc.UsersAddress));

  const badges = $derived.by<BadgeDef[]>(() => {
    const b: BadgeDef[] = [{ label: t("doc.type.vehicle"), tone: "accent", icon: "car" }];
    const expired = isPast(doc.ExpiryDate);
    if (expired === true) b.push({ label: t("doc.expired"), tone: "danger", icon: "alert" });
    else if (expired === false) b.push({ label: t("doc.validUntil", doc.ExpiryDate), tone: "success", icon: "check" });
    if (doc.VehicleCategory) b.push({ label: doc.VehicleCategory, tone: "neutral" });
    return b;
  });

  const sections = $derived.by<SectionDef[]>(() => {
    const s: SectionDef[] = [
      {
        title: t("vehicle.documentInformation"),
        icon: "file",
        fields: [
          { label: t("vehicle.stateIssuing"), value: doc.StateIssuing },
          { label: t("vehicle.authorityIssuing"), value: doc.AuthorityIssuing },
          { label: t("vehicle.issuingDate"), value: doc.IssuingDate },
          { label: t("vehicle.expiryDate"), value: doc.ExpiryDate },
          { label: t("vehicle.competentAuthority"), value: doc.CompetentAuthority, wide: true },
          { label: t("vehicle.unambiguousNumber"), value: doc.UnambiguousNumber, mono: true },
          { label: t("vehicle.serialNumber"), value: doc.SerialNumber, mono: true },
        ],
      },
      {
        title: t("vehicle.vehicleInformation"),
        icon: "car",
        fields: [
          { label: t("vehicle.registrationNumberOfVehicle"), value: doc.RegistrationNumberOfVehicle, mono: true },
          { label: t("vehicle.dateOfFirstRegistration"), value: doc.DateOfFirstRegistration },
          { label: t("vehicle.vehicleMake"), value: doc.VehicleMake },
          { label: t("vehicle.commercialDescription"), value: doc.CommercialDescription },
          { label: t("vehicle.vehicleIdNumber"), value: doc.VehicleIdNumber, mono: true, wide: true },
          { label: t("vehicle.vehicleType"), value: doc.VehicleType },
          { label: t("vehicle.category"), value: doc.VehicleCategory },
          { label: t("vehicle.colourOfVehicle"), value: doc.ColourOfVehicle },
          { label: t("vehicle.yearOfProduction"), value: doc.YearOfProduction },
          { label: t("vehicle.typeApprovalNumber"), value: doc.TypeApprovalNumber, mono: true, optional: true },
          { label: t("vehicle.homologationMark"), value: doc.HomologationMark },
        ],
      },
      {
        title: t("vehicle.ownerInformation"),
        icon: "user",
        fields: [
          { label: t("vehicle.ownersSurnameOrBusinessName"), value: doc.OwnersSurnameOrBusinessName },
          { label: t("vehicle.ownerName"), value: doc.OwnerName },
          { label: ownerNoLabel, value: doc.OwnersPersonalNo, mono: true, wide: true },
          { label: t("vehicle.ownerAddress"), value: doc.OwnerAddress, wide: true },
        ],
      },
    ];
    if (hasUser) {
      s.push({
        title: t("vehicle.userInformation"),
        icon: "user",
        fields: [
          { label: t("vehicle.usersSurnameOrBusinessName"), value: doc.UsersSurnameOrBusinessName },
          { label: t("vehicle.usersName"), value: doc.UsersName },
          { label: userNoLabel, value: doc.UsersPersonalNo, mono: true, wide: true },
          { label: t("vehicle.usersAddress"), value: doc.UsersAddress, wide: true },
        ],
      });
    }
    s.push({
      title: t("vehicle.technicalInformation"),
      icon: "sliders",
      fields: [
        { label: t("vehicle.maximumNetPower"), value: doc.MaximumNetPower, suffix: "kW" },
        { label: t("vehicle.engineCapacity"), value: doc.EngineCapacity, suffix: "cm³" },
        { label: t("vehicle.engineIdNumber"), value: doc.EngineIdNumber, mono: true },
        { label: t("vehicle.typeOfFuel"), value: doc.TypeOfFuel },
        { label: t("vehicle.vehicleMass"), value: doc.VehicleMass, suffix: "kg" },
        { label: t("vehicle.maximumPermissibleLadenMass"), value: doc.MaximumPermissibleLadenMass, suffix: "kg" },
        { label: t("vehicle.load"), value: doc.VehicleLoad, suffix: "kg" },
        { label: t("vehicle.powerWeightRatio"), value: doc.PowerWeightRatio, suffix: "kW/kg" },
        { label: t("vehicle.numberOfAxles"), value: doc.NumberOfAxles },
        { label: t("vehicle.numberOfSeats"), value: doc.NumberOfSeats },
        { label: t("vehicle.numberOfStandingPlaces"), value: doc.NumberOfStandingPlaces },
      ],
    });
    return s;
  });
</script>

<DocHero
  title={[doc.VehicleMake, doc.CommercialDescription].filter(Boolean).join(" ")}
  subtitle={[doc.VehicleType, doc.ColourOfVehicle, doc.YearOfProduction].filter(Boolean).join(" · ") || undefined}
  {badges}
  facts={[
    {
      label: t("vehicle.ownersSurnameOrBusinessName"),
      value: [doc.OwnersSurnameOrBusinessName, doc.OwnerName].filter(Boolean).join(" "),
    },
    { label: t("vehicle.vehicleIdNumber"), value: doc.VehicleIdNumber, mono: true },
    { label: t("vehicle.dateOfFirstRegistration"), value: doc.DateOfFirstRegistration },
  ]}
>
  {#snippet visual()}
    <div class="plate" aria-label={t("vehicle.registrationNumberOfVehicle")}>
      <div class="eu">
        <svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true">
          {#each Array(12) as _, i (i)}
            <circle
              cx={10 + 7 * Math.cos((i * Math.PI) / 6)}
              cy={10 + 7 * Math.sin((i * Math.PI) / 6)}
              r="1.1"
              fill="#ffd34d"
            />
          {/each}
        </svg>
        <span>SRB</span>
      </div>
      <div class="reg">{doc.RegistrationNumberOfVehicle || "—"}</div>
    </div>
  {/snippet}
</DocHero>

<DocSections {sections} />

<style>
  .plate {
    display: flex;
    align-items: stretch;
    width: 200px;
    height: 56px;
    border-radius: 8px;
    overflow: hidden;
    background: #fff;
    border: 2.5px solid #1c2230;
    box-shadow: var(--shadow);
    transform: rotate(-3deg);
  }
  .eu {
    width: 30px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    background: #1f4fb8;
    color: #fff;
    font-size: 9.5px;
    font-weight: 700;
  }
  .reg {
    flex: 1;
    display: grid;
    place-items: center;
    font-family: "DIN Alternate", "Bahnschrift", var(--mono);
    font-size: 25px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #121621;
    white-space: nowrap;
    padding: 0 6px;
  }
  @media (max-width: 760px) {
    .plate {
      width: 160px;
      height: 46px;
    }
    .reg {
      font-size: 19px;
    }
  }
</style>

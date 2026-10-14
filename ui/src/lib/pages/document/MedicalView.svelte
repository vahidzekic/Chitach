<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import DocHero from "./DocHero.svelte";
  import DocSections from "./DocSections.svelte";
  import type { MedicalDocument } from "../../api/types";
  import { t, yesNo } from "../../i18n/index.svelte";
  import { isPast, joinComma } from "../../util/format";
  import type { BadgeDef, SectionDef } from "./fields";

  let { doc }: { doc: MedicalDocument } = $props();

  const fullName = $derived(joinComma(doc.GivenNameLatin, doc.ParentNameLatin, doc.FamilyNameLatin));

  const badges = $derived.by<BadgeDef[]>(() => {
    const b: BadgeDef[] = [{ label: t("doc.type.medical"), tone: "accent", icon: "medical" }];
    if (doc.PermanentlyValid) {
      b.push({ label: t("doc.permanentlyValid"), tone: "success", icon: "check" });
    } else {
      const past = isPast(doc.ValidUntil);
      if (past === true) b.push({ label: t("doc.validationExpired"), tone: "warning", icon: "alert" });
      else if (past === false)
        b.push({ label: t("doc.validationValid", doc.ValidUntil), tone: "success", icon: "check" });
    }
    return b;
  });

  const sections = $derived<SectionDef[]>([
    {
      title: t("medical.generalInformation"),
      icon: "user",
      fields: [
        { label: t("medical.fullName"), value: fullName, wide: true },
        { label: t("medical.familyName"), value: doc.FamilyName, optional: true },
        { label: t("medical.givenName"), value: doc.GivenName, optional: true },
        { label: t("medical.gender"), value: doc.Gender },
        { label: t("medical.dateOfBirth"), value: doc.DateOfBirth },
        { label: t("medical.personalNumber"), value: doc.PersonalNumber, mono: true },
        { label: t("medical.insuranceNumber"), value: doc.InsurantNumber, mono: true },
      ],
    },
    {
      title: t("medical.cardInformation"),
      icon: "card",
      note: t("medical.validationNote"),
      fields: [
        { label: t("medical.cardId"), value: doc.CardId, mono: true },
        { label: t("medical.chipSerialNumber"), value: doc.ChipSerialNumber, mono: true },
        { label: t("medical.dateOfIssue"), value: doc.DateOfIssue },
        { label: t("medical.dateOfExpiry"), value: doc.DateOfExpiry },
        { label: t("medical.validUntil"), value: doc.ValidUntil },
        { label: t("medical.permanentlyValid"), value: yesNo(doc.PermanentlyValid) },
        { label: t("medical.insurerName"), value: doc.InsurerName, optional: true, wide: true },
        { label: t("medical.insurerId"), value: doc.InsurerID, optional: true },
        { label: t("medical.printLanguage"), value: doc.PrintLanguage, optional: true },
      ],
    },
    {
      title: t("medical.address"),
      icon: "globe",
      fields: [
        { label: t("medical.street"), value: doc.Street },
        { label: t("medical.streetCode"), value: doc.StreetCode },
        { label: t("medical.number"), value: doc.Number },
        { label: t("medical.entrance"), value: doc.Entrance },
        { label: t("medical.apartment"), value: doc.Apartment },
        { label: t("medical.postNumber"), value: doc.PostNumber },
        { label: t("medical.place"), value: doc.Place },
        { label: t("medical.municipality"), value: doc.Municipality },
        { label: t("medical.country"), value: doc.Country },
      ],
    },
    {
      title: t("medical.insuranceCarrierInformation"),
      icon: "shield",
      fields: [
        {
          label: t("medical.carrier"),
          value: [doc.CarrierGivenNameLatin, doc.CarrierFamilyNameLatin].filter(Boolean).join(" "),
          wide: true,
        },
        { label: t("medical.insuranceNumber"), value: doc.CarrierInsurantNumber, mono: true },
        { label: t("medical.personalNumber"), value: doc.CarrierIdNumber, mono: true },
        { label: t("medical.familyMember"), value: yesNo(doc.CarrierFamilyMember) },
        { label: t("medical.relationship"), value: doc.CarrierRelationship },
      ],
    },
    {
      title: t("medical.insuranceInformation"),
      icon: "checkCircle",
      fields: [
        { label: t("medical.insuranceBasis"), value: doc.InsuranceBasisRZZO, mono: true },
        { label: t("medical.insuranceStartDate"), value: doc.InsuranceStartDate },
        { label: t("medical.insuranceDescription"), value: doc.InsuranceDescription, wide: true },
      ],
    },
    {
      title: t("medical.taxpayerInformation"),
      icon: "table",
      fields: [
        { label: t("medical.taxpayerName"), value: doc.TaxpayerName, wide: true },
        { label: t("medical.taxpayerResidence"), value: doc.TaxpayerResidence },
        { label: t("medical.taxpayerActivityCode"), value: doc.TaxpayerActivityCode, mono: true },
        { label: t("medical.taxpayerNumber"), value: doc.TaxpayerNumber, mono: true },
        { label: t("medical.taxpayerIdNumber"), value: doc.TaxpayerIdNumber, mono: true },
      ],
    },
  ]);

  const initials = $derived(
    ((doc.GivenNameLatin?.[0] ?? "") + (doc.FamilyNameLatin?.[0] ?? "")).toUpperCase(),
  );
</script>

<DocHero
  title={[doc.GivenNameLatin, doc.FamilyNameLatin].filter(Boolean).join(" ")}
  subtitle={[doc.GivenName, doc.FamilyName].filter(Boolean).join(" ") || undefined}
  {badges}
  facts={[
    { label: t("medical.insuranceNumber"), value: doc.InsurantNumber, mono: true },
    { label: t("medical.personalNumber"), value: doc.PersonalNumber, mono: true },
    { label: t("medical.validUntil").replace("*", ""), value: doc.ValidUntil },
  ]}
>
  {#snippet visual()}
    <div class="avatar" aria-hidden="true">
      {#if initials}<span>{initials}</span>{:else}<Icon name="medical" size={40} />{/if}
      <div class="cross"><Icon name="medical" size={16} stroke={2.2} /></div>
    </div>
  {/snippet}
</DocHero>

<DocSections {sections} />

<style>
  .avatar {
    position: relative;
    width: 112px;
    height: 112px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, #19b38a, #0e8f86);
    color: #fff;
    font-size: 38px;
    font-weight: 700;
    letter-spacing: 0.02em;
    border: 4px solid var(--surface);
    box-shadow: var(--shadow);
  }
  .cross {
    position: absolute;
    right: -2px;
    bottom: 4px;
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--surface);
    color: #e5484d;
    box-shadow: var(--shadow);
  }
  @media (max-width: 760px) {
    .avatar {
      width: 88px;
      height: 88px;
      font-size: 30px;
    }
  }
</style>

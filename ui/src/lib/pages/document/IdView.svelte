<script lang="ts">
  import Icon from "../../components/Icon.svelte";
  import DocHero from "./DocHero.svelte";
  import DocSections from "./DocSections.svelte";
  import type { IdDocument } from "../../api/types";
  import { t } from "../../i18n/index.svelte";
  import { isPast, joinComma } from "../../util/format";
  import type { BadgeDef, SectionDef } from "./fields";

  let { doc }: { doc: IdDocument } = $props();

  // Port of IdDocument::full_address (crates/celik-core).
  function fullAddress(d: IdDocument): string {
    let street = d.Street;
    if (d.HouseNumber || d.HouseLetter || d.Entrance) {
      street += " " + d.HouseNumber + d.HouseLetter;
      if (d.Floor) street += "/" + d.Floor;
      if (d.ApartmentNumber) street += "/" + d.ApartmentNumber;
    }
    return joinComma(street, d.Community, d.Place);
  }

  function typeKey(type: string): string {
    switch (type) {
      case "ID":
        return "doc.type.id";
      case "IF":
        return "doc.type.idForeigner";
      case "RP":
        return "doc.type.residencePermit";
      default:
        return "doc.type.idApollo";
    }
  }

  const foreigner = $derived(doc.DocumentType === "IF" || doc.DocumentType === "RP");
  const rp = $derived(doc.DocumentType === "RP");

  const badges = $derived.by<BadgeDef[]>(() => {
    const b: BadgeDef[] = [{ label: t(typeKey(doc.DocumentType)), tone: "accent", icon: "card" }];
    const expired = isPast(doc.ExpiryDate);
    if (expired === true) b.push({ label: t("doc.expired"), tone: "danger", icon: "alert" });
    else if (expired === false) b.push({ label: t("doc.validUntil", doc.ExpiryDate), tone: "success", icon: "check" });
    return b;
  });

  const sections = $derived<SectionDef[]>([
    {
      title: t("id.citizenInformation"),
      icon: "user",
      fields: [
        { label: t("id.surname"), value: doc.Surname },
        { label: t("id.givenName"), value: doc.GivenName },
        { label: t("id.parentGivenName"), value: doc.ParentGivenName },
        { label: t("id.sex"), value: doc.Sex },
        { label: t("id.birthDate"), value: doc.DateOfBirth },
        { label: t("id.personalNumber"), value: doc.PersonalNumber, mono: true },
        {
          label: t("id.birthPlace"),
          value: joinComma(doc.PlaceOfBirth, doc.CommunityOfBirth, doc.StateOfBirth),
          wide: true,
        },
        { label: t("id.nationalityFull"), value: doc.NationalityFull, optional: !foreigner, wide: true },
        { label: t("id.foreignerStatus"), value: doc.StatusOfForeigner, optional: true, wide: true },
        { label: t("id.purposeOfStay"), value: doc.PurposeOfStay, optional: !rp, wide: true },
        { label: t("id.eNote"), value: doc.ENote, optional: !rp, wide: true },
      ],
    },
    {
      title: t("id.residence"),
      icon: "globe",
      fields: [
        { label: t("id.address"), value: fullAddress(doc), wide: true },
        { label: t("id.place"), value: doc.Place },
        { label: t("id.community"), value: doc.Community },
        { label: t("id.street"), value: doc.Street },
        { label: t("id.houseNumber"), value: doc.HouseNumber + doc.HouseLetter },
        { label: t("id.entrance"), value: doc.Entrance, optional: true },
        { label: t("id.floor"), value: doc.Floor, optional: true },
        { label: t("id.apartmentNumber"), value: doc.ApartmentNumber, optional: true },
        { label: t("id.addressDate"), value: doc.AddressDate },
        { label: t("id.addressLabel"), value: doc.AddressLabel, optional: true },
      ],
    },
    {
      title: t("id.documentInformation"),
      icon: "card",
      fields: [
        { label: t("id.documentName"), value: doc.DocumentName, optional: !rp, wide: true },
        { label: t("id.issuingAuthority"), value: doc.IssuingAuthority, wide: true },
        { label: t("id.docRegNo"), value: doc.DocRegNo, mono: true },
        { label: t("id.issuingDate"), value: doc.IssuingDate },
        { label: t("id.expiryDate"), value: doc.ExpiryDate },
      ],
    },
    {
      title: t("id.technical"),
      icon: "chip",
      fields: [
        { label: t("id.documentType"), value: doc.DocumentType, optional: true },
        { label: t("id.documentSerialNumber"), value: doc.DocumentSerialNumber, mono: true, optional: true },
        { label: t("id.chipSerialNumber"), value: doc.ChipSerialNumber, mono: true, optional: true, wide: true },
        { label: t("id.stateOfBirthCode"), value: doc.StateOfBirthCode, optional: true },
        { label: t("id.state"), value: doc.State, optional: true },
      ],
    },
  ]);
</script>

<DocHero
  title={[doc.GivenName, doc.Surname].filter(Boolean).join(" ")}
  subtitle={doc.ParentGivenName ? `${t("id.parentGivenName")}: ${doc.ParentGivenName}` : undefined}
  {badges}
  facts={[
    { label: t("id.personalNumber"), value: doc.PersonalNumber, mono: true },
    { label: t("id.birthDate"), value: doc.DateOfBirth },
    { label: t("id.docRegNo"), value: doc.DocRegNo, mono: true },
  ]}
>
  {#snippet visual()}
    <div class="portrait">
      {#if doc.Portrait}
        <img src={`data:image/jpeg;base64,${doc.Portrait}`} alt={joinComma(doc.GivenName, doc.Surname)} />
      {:else}
        <Icon name="user" size={56} stroke={1.2} />
      {/if}
    </div>
  {/snippet}
</DocHero>

<DocSections {sections} />

<style>
  .portrait {
    width: 132px;
    aspect-ratio: 300 / 380;
    border-radius: 12px;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--surface-3);
    color: var(--text-3);
    border: 3px solid var(--surface);
    box-shadow: var(--shadow);
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  @media (max-width: 760px) {
    .portrait {
      width: 104px;
    }
  }
</style>

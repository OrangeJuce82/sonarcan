<script lang="ts">
  import sonarcanLogo from "../../docs/assets/sonarcan-rounded.png";
  import type { ExternalLinkTarget } from "./backend";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";
  import type { AboutCopy } from "./aboutI18n";

  let { copy, version, close, openLink }: {
    copy: AboutCopy;
    version: string;
    close: () => void;
    openLink: (target: ExternalLinkTarget) => void;
  } = $props();
</script>

<Modal title={copy.title} closeLabel={copy.close} {close} wide>
  <div class="about">
    <section class="about-identity">
      <img src={sonarcanLogo} alt="" />
      <div>
        <h3>SonArcan</h3>
        <p>{copy.description}</p>
        <small>{copy.localFirst}</small>
        <span>Version {version}</span>
      </div>
    </section>

    <section class="about-section">
      <h3>{copy.toolsTitle}</h3>
      <p>{copy.toolsDescription}</p>
      <ul class="about-tools">
        <li>{copy.audioTools}</li>
        <li>{copy.analysisTools}</li>
        <li>{copy.desktopTools}</li>
      </ul>
    </section>

    <section class="about-section">
      <h3>{copy.licensesTitle}</h3>
      <p>{copy.licensesDescription}</p>
      <div class="about-links">
        <button onclick={() => openLink("github")}><Icon name="github" size="14px" />{copy.sourceCode}</button>
        <button onclick={() => openLink("github-issues")}><Icon name="circle-info" size="14px" />{copy.reportIssue}</button>
        <button onclick={() => openLink("licenses")}><Icon name="file" size="13px" />{copy.thirdPartyNotices}</button>
      </div>
    </section>

    <section class="about-section about-thanks">
      <h3>{copy.thanksTitle}</h3>
      <p>{copy.thanksDescription}</p>
    </section>
  </div>
  <div class="modal-actions"><button class="primary" onclick={close}>{copy.close}</button></div>
</Modal>

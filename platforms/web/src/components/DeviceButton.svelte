<script lang="ts">
  // The device button (docs/ui.md, "Links and remote playback"): while a remote source plays it
  // opens Devices, every device with the playing one checked; choosing one moves the music there.
  // The bar shows it only while a remote source plays. Jellyfesh Web plays none yet (remote
  // sources are refused at install, ADR-0018), so in the web it stays hidden.
  import type { RemoteDevices } from '../lib/core/types';
  import { t } from '../lib/i18n/index.svelte';
  import Menu from './Menu.svelte';

  let { remote, onchoose }: { remote: RemoteDevices; onchoose: (id: string) => void } = $props();
</script>

<Menu
  icon="devices"
  label={t('Devices')}
  items={() => remote.devices.map((device) => ({ label: device.name, checked: device.active, action: () => onchoose(device.id) }))}
/>

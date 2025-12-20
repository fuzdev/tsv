<script lang="ts">
  // Basic v flag (ES2024 unicodeSets)
  const a = /[a-z]/v;
  const b = /pattern/vigd;
  const c = /pattern/smigv;

  // Unicode property escapes with v flag
  const d = /[\p{Letter}]/v;
  const e = /\p{RGI_Emoji}/v;
  const f = /\p{RGI_Emoji_Tag_Sequence}/v;
</script>

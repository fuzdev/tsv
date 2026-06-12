<script lang="ts">
	let a = $state('中');
</script>

<!-- Element, attribute, and directive name_loc positions (column + character)
     after multibyte text on the same line must translate to UTF-16 offsets. -->
中文 <input data-attr="value" bind:value={a} />
<p>😀 <span data-attr="value">text</span></p>

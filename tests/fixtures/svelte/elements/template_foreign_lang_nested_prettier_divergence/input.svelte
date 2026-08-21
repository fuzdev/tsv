<!-- A foreign `<template lang="…">` body keeps the author's own columns at every nesting
	depth — the language is whitespace-significant, so its lines only mean anything relative
	to each other. -->
<div>
	<template lang="coffee">
if cond
  x = 1
	</template>
</div>

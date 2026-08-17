<!-- A comment in a foreign `<template lang="…">` head takes the ordinary attribute-list
	layout, so the `>` drops to base indent whenever the head breaks. The template's own
	content stays raw. -->

<!-- Trailing line comment: the head breaks and `>` takes its own line -->
<template
	   lang="pug"   // c
>
h1 text
</template>

<!-- Trailing block comment is self-delimiting, so the head stays on one line -->
<template    lang="pug"    /* c */   >
h1 text
</template>

<!-- Own-line comment before the `>` keeps its own line -->
<template
	   lang="pug"
	   /* c */
>
h1 text
</template>

<script lang="ts"></script>

<!-- `catch` starts no expression, so the type argument list before a shorthand `catch` stays
	the head's -->
{#await (f<T>) catch error}{error}{/await}
{#await (f()<T>) catch error}{error}{/await}
{#await (a.b<T>) catch}text{/await}
{#await (f<T, U>) catch error}{error}{/await}

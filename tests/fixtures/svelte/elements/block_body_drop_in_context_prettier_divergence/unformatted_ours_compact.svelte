<!-- real breadcrumb shape: an {#each} whose breakable <a> body follows an inline <a> sibling — the body drops to its own line (prettier hugs the } and breaks the <a> internally) -->
<nav>
	<a href={root}>Home</a>{#each path_pieces as piece (piece)}<a href={base + piece.path} class={{selected: piece.selected}}>{piece.name}</a>{/each}
</nav>

<!-- if/else with breakable branches, nested two inline <span> wrappers deep — both branch bodies drop -->
<p>
	<span class="label">Status</span><span class="value">{#if loading}<Spinner size={spinner_size} variant={spinner_variant} />{:else}<Badge tone={badge_tone}>{badge_text}</Badge>{/if}</span>
</p>

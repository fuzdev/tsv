<style>
	/* leading comment before url */
	@import    /* a */    url('a.css');

	/* leading comment before bare string */
	@import    /* b */    'b.css';

	/* comment after url, before media type */
	@import url('c.css')    /* c */    screen;

	/* comments around comma in media-query list */
	@import url('d.css')    /* d */    projection    /* e */    ,    /* f */    tv;

	/* comment before bare layer keyword */
	@import url('e.css')    /* g */    layer    /* h */    screen;
</style>

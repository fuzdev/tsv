<style>
div {
/* 100 chars - stays inline (at boundary) */
filter: drop-shadow(000000000000000px 00000000000000000px 00000000000000000px rgba(0,0,0,0));

/* 101 chars - wraps */
filter: drop-shadow(0000000000000000px 00000000000000000px 00000000000000000px rgba(0,0,0,0));

/* url filter with long path - single-arg function, doesn't wrap */
filter: url('00000000000000000000000000000000000000000000000000000000000000000000000000000000000');
}
</style>

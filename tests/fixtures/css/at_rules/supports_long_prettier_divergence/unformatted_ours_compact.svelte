<style>
/* @supports wraps at 101 chars (we wrap at >100, Prettier waits until >102) */

/* 100 chars - stays inline (both agree) */
@supports (display: grid) and (transform: rotate(45deg)) and (filter: blur(5px)) and (flex: 1aa) {
.a {
color: red;
}
}

/* 101 chars - we wrap, Prettier stays inline */
@supports (display: grid) and (transform: rotate(45deg)) and (filter: blur(5px)) and (flex: 1aaa) {
.b {
color: blue;
}
}

/* 102 chars - we wrap, Prettier stays inline */
@supports (display: grid) and (transform: rotate(45deg)) and (filter: blur(5px)) and (flex: 1aaaa) {
.c {
color: green;
}
}

/* 103 chars - wraps (both agree) */
@supports (display: grid) and (transform: rotate(45deg)) and (filter: blur(5px)) and (flex: 1aaaaa) {
.d {
color: yellow;
}
}

/* very long - the continuation re-wraps greedily across multiple lines (both agree) */
@supports (aaaaaaaaaaaaaaaa: 1) and (bbbbbbbbbbbbbbbb: 1) and (cccccccccccccccc: 1) and (dddddddddddddddd: 1) and (eeeeeeeeeeeeeeee: 1) and (ffffffffffffffff: 1) and (gggggggggggggggg: 1) {
.e {
color: orange;
}
}
</style>

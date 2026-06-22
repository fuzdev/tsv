<!-- Await block inside inline element - 100/101 char boundary tests -->

<!-- 100 chars exactly - stays inline -->
<a>{#await promise}text_padding_to_reach_exactly_one_hundred_chars_abcdefghijklmnopqrstu{/await}</a>

<!-- 101 chars - lays out block-style (a intact, await on its own indented line) -->
<a>{#await promise}text_padding_to_reach_exactly_one_hundred_chars_abcdefghijklmnopqrstuv{/await}</a>

<!-- span element at 100 chars - stays inline -->
<span>{#await promise}text_to_reach_exactly_one_hundred_characters_abcdefghijklmnopqr{/await}</span>

<!-- span at 101 chars - breaks -->
<span>{#await promise}text_to_reach_exactly_one_hundred_characters_abcdefghijklmnopqrs{/await}</span>

<!-- With :then branch - 100 chars exactly -->
<a>{#await promise}text{:then val}padding_to_reach_exactly_one_hundred_abcdefghijklmnopq{/await}</a>

<!-- With :then branch - 101 chars -->
<a>{#await promise}text{:then val}padding_to_reach_exactly_one_hundred_abcdefghijklmnopqr{/await}</a>

<!-- Basic escaped angle brackets -->
{'<'}div{'>'}
{'<'}code{'>'}{'<'}/code{'>'}

<!-- With surrounding text -->
Text before {'<'}div{'>'} text after

<!-- Only trailing text -->
{'<'}span{'>'} text after

<!-- Only leading text -->
Text before {'<'}span{'>'}

<!-- Multiple pairs with content -->
{'<'}p{'>'}paragraph{'<'}/p{'>'}

<!-- Self-closing pattern -->
{'<'}br/{'>'}

<!-- With attribute text -->
{'<'}div class="test"{'>'}

<!-- Inside inline element -->
<code>{'<'}div{'>'}</code>

<!-- With expression in middle -->
{'<'}Component {prop}{'/>'}

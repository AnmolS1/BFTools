// ROT13 cipher
// Reads characters from stdin and applies ROT13, writing to stdout.
// Non-alphabetic characters are passed through unchanged.
-,+[                         // Read first character; loop while not EOF (0)
    -[-[-[-[-[-[-[-[-[-[-[-[-[
        >>++++[>++++++++<-]  // Divisor = 32
        >[
            <+++++++++++++   // Add 13 for A-M, N-Z
            [->>+<<]         // Move to result
            >>[-<<+>>]       // Move back
            <<
        ]
        >[-]
    ]]]]]]]]]]]]]
    ,+                       // Read next character
]

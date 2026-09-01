help_mappings = %w(-h -? --help -D)
symbols = %i(a b c)
bracket_delim = %w[a b]
empty = %w()
escaped_space = %w(foo\ bar baz)
interpolated_words = %W(a#{b} c)
interpolated_symbols = %I(a#{b} c)
thor_reserved_words = %w(invoke
                        shell
                        options
                        behavior
                        root
                        destination_root
                        relative_root
                        action
                        add_file
                        create_file
                        in_root
                        inside
                        run
                        run_ruby_script)

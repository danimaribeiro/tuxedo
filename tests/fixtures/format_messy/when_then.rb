case key
when :organization_identifier then "the branch below is written with `then` but does not fit on the line"
when :organization_display_name then lookup_table_for_organizations.fetch(value).display_name_for_locale(current_locale, fallback: true)
when :two_statements then a; b
when :written_out then
  :c
when :block_construct then if a then b end
end

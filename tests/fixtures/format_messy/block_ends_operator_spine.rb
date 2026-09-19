class Command
  def fits
    short? && begin
      work
    end
  end

  def fits_with_header
    first_condition_here?(instance) && second_condition_here?(instance) && third_cond? && begin
      work
    end
  end

  def breaks_begin
    not_debugging?(instance) && (error.message =~ /wrong number of arguments/ || error.message =~ /given \d*, expected \d*/) && begin
      saned = sans_backtrace(error.backtrace, caller)
      saned.empty? || saned.size == 1
    end
  end

  def breaks_if
    not_debugging?(instance) && (error.message =~ /wrong number of arguments/ || error.message =~ /given \d*, expected \d*/) && if foo
      bar
    end
  end

  def breaks_case
    not_debugging?(instance) && (error.message =~ /wrong number of arguments/ || error.message =~ /given \d*, expected \d*/) && case foo
    when 1 then 2
    end
  end
end

puts "yes" if active
yield unless pretend?
return if done?
raise "error" unless valid?

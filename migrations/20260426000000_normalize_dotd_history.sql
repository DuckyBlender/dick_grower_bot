-- Normalize legacy Dick of the Day history values from older naming.
UPDATE length_history
SET growth_type = 'dotd'
WHERE growth_type IN ('sotd', 'schlong_of_day', 'schlongoftheday');

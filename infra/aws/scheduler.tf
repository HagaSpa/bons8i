resource "aws_scheduler_schedule" "probe" {
  name                = "bons8i-external-probe"
  schedule_expression = "rate(10 minutes)"

  flexible_time_window {
    mode = "OFF"
  }

  target {
    arn      = aws_lambda_function.probe.arn
    role_arn = aws_iam_role.scheduler.arn

    retry_policy {
      maximum_retry_attempts = 0
    }
  }
}

use crate::backtest::BacktestResult;
use crate::triangular_arbitrage::ArbitragePath;

pub struct ReportGenerator;

impl ReportGenerator {
    pub fn print_backtest_report(result: &BacktestResult) {
        println!("\n{}", "=".repeat(80));
        println!("  📊 TRIANGULAR ARBITRAGE BACKTEST REPORT");
        println!("{}", "=".repeat(80));
        
        println!("\n📈 Performance Metrics:");
        println!("  Total Updates Processed:    {}", result.total_updates_processed);
        println!("  Total Opportunities Found:  {}", result.total_opportunities);
        println!("  Execution Time:             {} ms", result.execution_time_ms);
        println!("  Updates per Second:         {:.0}", 
            result.total_updates_processed as f64 / (result.execution_time_ms as f64 / 1000.0));
        
        println!("\n💰 Profit Analysis:");
        println!("  Total Profit:               ${:.2}", result.total_profit);
        println!("  Average Profit per Opp:     ${:.2}", result.avg_profit_per_opportunity);
        
        if let Some(best) = &result.best_opportunity {
            println!("\n🏆 Best Opportunity:");
            println!("  Timestamp:                  {}", best.timestamp);
            println!("  Path:                       {:?}", best.path);
            println!("  Profit Percentage:          {:.4}%", best.profit_percentage);
            println!("  Net Profit:                 ${:.2}", best.net_profit);
            println!("  Input Amount:               ${:.2}", best.input_amount);
            println!("  Expected Output:            ${:.2}", best.expected_output);
        } else {
            println!("\n⚠️  No opportunities found!");
        }
        
        println!("\n{}", "=".repeat(80));
    }

    pub fn generate_csv_report(result: &BacktestResult, filename: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(filename)?;
        
        writeln!(file, "Backtest Summary Report")?;
        writeln!(file, "=======================")?;
        writeln!(file, "Total Updates,{}", result.total_updates_processed)?;
        writeln!(file, "Total Opportunities,{}", result.total_opportunities)?;
        writeln!(file, "Total Profit,{:.2}", result.total_profit)?;
        writeln!(file, "Average Profit,{:.2}", result.avg_profit_per_opportunity)?;
        writeln!(file, "Execution Time (ms),{}", result.execution_time_ms)?;
        
        if let Some(best) = &result.best_opportunity {
            writeln!(file, "\nBest Opportunity")?;
            writeln!(file, "Timestamp,{}", best.timestamp)?;
            writeln!(file, "Path,{:?}", best.path)?;
            writeln!(file, "Profit %,{:.4}", best.profit_percentage)?;
            writeln!(file, "Net Profit,{:.2}", best.net_profit)?;
        }
        
        Ok(())
    }
}
